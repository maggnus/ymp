# Small weak-agent versus strong-agent pilot

Status: preparation in progress, requested by the owner on 2026-09-14 under
YMP-201. The owner asked for a simple real trial and a scientifically grounded
account of whether two or three weaker agents can do the work of one stronger
agent. This narrows the first study; it does not start the older broad experiment
program or remove the earlier requirement to approve concrete settings and a
resource envelope. No measured native run has started and no quota is inferred
from historical proposals.

## Question and interpretation

Can two or three participants using a lower-capability model reach the same
externally checked outcome as one higher-capability model, on the declared small
tasks and within the declared total allowance? Distinguish reaching the result,
spending fewer resources, and finishing sooner. They are separate claims.

The discovered candidate pair is gpt-5.6-luna and gpt-6-astra via the installed
Codex model catalog. Both offer low effort. This is a proposed comparison, not a
measured capability ordering or a guarantee of current quota. Record exact native
versions and requested, sent and reported controls. A weak-solo condition checks
whether the selected cases exhibit a capability gap at all.

## Scientific basis and competing explanations

[Li et al., More Agents Is All You Need, 2024](https://arxiv.org/abs/2402.05120)
report benefits from sampling and voting, including smaller models approaching
larger ones on selected benchmarks. Their illustrated comparisons use roughly
15--20 samples, so they do not establish that two or three suffice here. The
study motivates an independent-attempt control rather than assuming collaboration
caused every benefit.

[Choi et al., Debate or Vote, 2025](https://arxiv.org/abs/2508.17536v2)
separate voting from debate across seven NLP benchmarks. Voting explains much of
the measured gain; targeted corrective interventions can improve debate. Their
theoretical result depends on their stochastic belief-update assumptions and is
not a proof that every tool-using multi-agent protocol is ineffective.

[Kim et al., Towards a Science of Scaling Agent Systems, 2026 revision](https://arxiv.org/abs/2512.08296v3)
compare agentic architectures under controlled conditions. Their reported effects
vary markedly with task structure: decomposable work can improve while sequential
work can degrade. This supports testing bounded coordination with external
checks, not treating agent count as an unconditional source of capability.

The proposed mechanism is independent initial work, followed by a short,
evidence-driven correction step. Two participants can implement and independently
check; three can produce two alternatives and have the third check and integrate.
The third participant must use the weak model too. No strong agent may solve,
select or repair an experimental team result behind the scenes. Strong agents
may prepare the study, but preparation is recorded separately from execution.

Under an illustrative independent binary-vote model with equal individual
correctness p, three-vote correctness is 3p^2 - 2p^3. For p=0.6 this is 0.648,
which is still below a strong comparator at 0.8. Correlated errors violate the
independence assumption. This elementary calculation explains why quantity can
help yet fail to close a large gap; it is not a prediction for the actual models.

## Minimum conditions to freeze before native execution

The intended small matrix has six conditions: strong solo; weak solo; two and
three isolated weak attempts with a declared selection rule; two and three weak
participants that may interact. Use two small, externally checkable tasks as the
initial diagnostic unit, for 12 outcome attempts rather than the old 24-run
proposal. Separate task variants used during preparation from measured inputs.
The actual executable matrix and allowance must be finalized after checking the
existing native runner and identity/accounting boundaries.

All conditions get identical requirements, visible inputs, permitted tools and
aggregate resource ceilings. Solo keeps its complete native tool loop and may
self-correct. A group shares one allowance; it does not receive the solo allowance
per member. Charge generation, review, selection, coordination and every failed
attempt. Record input, cached input, output, time and coverage by originating agent.
Cross-model token volume is not monetary cost; unknown cost remains unknown.

Reuse the existing evaluation fixtures and validators where they can discriminate
errors. Suitable small cases are integer CSV reconciliation with exceptions and
a compact behavioral repair with edge cases. Freeze hidden evaluation separately
from agent-visible examples. Verify that the evaluator rejects plausible wrong
outputs. Avoid an all-trivial suite whose universal success cannot reveal a
capability difference.

Initial contexts in independent conditions must not expose peer answers or the
hidden evaluator. Distinct native handles do not by themselves establish
statistical independence. Select outputs by a frozen public-evidence rule; hidden
correctness cannot be used as an oracle to choose the winning artifact. Charge
any model-based selector to the corresponding condition and participant count.
Keep accepted artifacts unchanged until external scoring.

For an ymp product claim, the cooperative condition must execute through the
actual runtime with truthful participant/assignment records and ordinary
acceptance. A standalone research protocol may test a mechanism, but must be
reported as such, not as measured ymp functionality. Do not weaken independence
or create false agent identities to make a condition executable. The small pilot
must not depend on completing the entire YMP-148 strategy framework if an existing
honest execution path is sufficient.

Randomize condition order within a task and retain the seed. Freeze prompts and
allowances before observing measured outcomes. Keep each task's failures and
operational interruptions. Do not repair failed experimental outputs manually.

## Result and follow-up rules

The first result is a paired diagnostic table: externally checked completion,
defects, complete resource observations, latency and interventions. Two tasks do
not establish statistical equivalence, general superiority or impossibility.
If both model baselines always succeed, report an overhead/control experiment,
not closure of a capability gap. If both fail, do not label the task a fair
capability comparison without examining the protocol.

Compare cooperation with isolated attempts at the same aggregate allowance.
If they perform alike, attribute any observed gain to repeated attempts unless
additional evidence shows an interaction benefit. Inspect whether failures arise
from shared errors, ineffective checking, information loss or exhausted allowance.
A revised mechanism is a new declared condition and uses held-out instances; do
not retune on the scored tasks and present the rerun as independent confirmation.

Success supplies a constructive example for the tested conditions. Failure can
reject a particular protocol at a stated allowance and task scope. It cannot
prove universal impossibility across all models, tasks and coordination schemes.
Any broader confirmation study requires a separately sized sample and approval.
