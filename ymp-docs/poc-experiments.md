# POC experiment direction

Owner direction, 2026-09-28: develop quickly in small, checked increments and first
establish whether the proposed cooperation mechanisms work. Start directly with
a homogeneous GPT team. This document records the experiment direction; the
approved domain model still governs architecture and scope, and task records own
implementation status, priorities and dependencies. Follow the existing detailed
task plan. Experiments require a working engine and their actual product consumer.

## Questions and evidence

The first product question is whether a small team can produce a checkable result
within its resource envelope, with observable responsibility and independent
review. Once agent-initiated operations exist, ask whether proposals, commitments
and context exchange make a useful difference from a fixed workflow under matched
conditions. A successful provider call or Scripted run alone answers neither
question about useful self-organization.

For each bounded run retain the task and exact input, criterion/check outcome,
result or output, provider/model versions, requested/sent/reported settings,
effective policies, journal references, spent/held/unknown usage, elapsed time and
user interventions. Record which cooperation mechanism was actually exercised and
what remains unavailable. Negative and inconclusive results are valid findings;
one successful pilot does not establish superiority or a causal benefit.

## Team progression

Run experiments one after another, in this order:

| Stage | Team | Immediate question |
| --- | --- | --- |
| 1 | Two distinct GPT agents using the same available model and supported settings; initially producer and independent reviewer. | Does the smallest accountable team complete the elementary scenario correctly, and where does coordination fail or add cost? |
| 2 | Distinct agents using different available GPT models. | Does changing model composition alter the observed result or cost on the same task? |
| 3 | Agents from different model families through implemented native adapters. | What changes when model-family diversity is introduced? |

Separate agent identities and assignment context preserve reviewer independence
even when the model is the same. Same-model agreement is not independent evidence
of correctness and does not itself raise a confirmation grade. There is no
single-agent prerequisite; a later control arm may be added only for a concrete
comparison question. The initial two-agent choice minimizes the first experiment;
it is not a permanent team-size restriction.

Owner direction, 2026-09-29: run native experiments on light Anthropic models,
keeping research speed and evidence quality in balance: a result without quality
may be false, and tests are not an objective of their own. The stage order above
is unchanged; the homogeneous stage now uses two distinct agents on the lowest-cost
discovered Anthropic model, recorded in
`experiments/homogeneous-claude-elementary/`. Earlier GPT runs remain retained
evidence and are neither repeated nor relabelled.

Discover actual offerings and supported settings at run time. Apply the standing
authorization and low-token preference in AGENTS.md, with finite limits fixed
before each run. Do not increase effort, roster or retry count automatically.

## Task progression

| Level | Small starting scenario | Observable check |
| --- | --- | --- |
| Elementary | Transform a short supplied list into precisely specified sorted JSON. | Compare the final bytes or parsed values with an independently specified expected result. |
| Simple | Implement one pure function in a disposable small fixture, with one new behavior and one preservation requirement. | A focused executable check distinguishes the requested behavior and preserved case. |
| Medium | Make a small change across two related modules with an explicit dependency and a preserved behavior. | Check the integrated result and attribute any proposal, handoff or revision that helped or failed. |

Increase difficulty only after the preceding run has an interpretable outcome.
When changing team composition, reuse an already understood small task before
increasing difficulty again. Keep task inputs, checks, resource envelope and
policies matched when comparing composition; change policies separately when
testing a coordination mechanism. Start with one bounded pilot per selected
configuration and repeat only to resolve a stated uncertainty. Do not require an
exhaustive combination of teams, models and tasks before learning from a result.

## Engine readiness

Complete the existing tasks in their planned order. This experiment direction
does not change task priorities or bypass prerequisites. A bounded adapter
demonstration is an engineering check, not the first team experiment or proof of
useful self-organization.

The first homogeneous-team product experiment uses the real
[accountable session](tasks/records/W1-0014.json) once its dependencies exist;
visual polish and later model-family adapters are not prerequisites. Fixed
producer/reviewer behavior establishes the initial team path. The
[comparison runner](tasks/records/W3-0009.json) later compares it with implemented
agent initiative. Begin that comparison with the smallest applicable scenario;
the existing browser and ambiguous-constraint scenario remains a later exercise.
Do not bypass admission, independent acceptance, resource accounting or dependency
gates to label an early demonstration a completed product experiment.

## Development checks

Prefer a real consumer scenario over test scaffolding that mirrors private code.
Add a code test when a regression, authority/accounting boundary or otherwise
unobserved acceptance condition needs durable evidence. Use the narrowest useful
check during development; run the repository's required checks before committing
code. Repeat broad checks only after a relevant change, failure or unresolved
concern. Scripted and protocol-fixture evidence remain separate from native runs.
