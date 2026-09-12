# Product value under discussion

Status: the owner approved [intent.md](../../intent.md) on 2026-09-12 and requested the [delivery plan](../tasks/plan.md). The examples and performance expectations below remain hypotheses, not measured achievements. The [team and effort policy](../architecture/team-and-effort-policy.md) operates within trusted runtime authority, assignment-scoped roles, bounded membership and fixed user constraints.

## Purpose

YMP organizes available native AI agents to produce independently assessable results within the user's time and resource constraints. Its product hypothesis is that selective collaboration and accumulated verified experience can improve the tradeoff between result quality, resource use, elapsed time and human effort.

Agent count, provider diversity, temporary coordination roles, an optional Oracle and fixed or adaptive effort are possible mechanisms within the approved protocol. Their inclusion must be justified by the desired outcome. Permanent agent hierarchy and inherited assignment authority are excluded by the approved intent. A useful run may use one executor or several; a configured pool does not require activating every agent.

## Five kinds of value

| Desired improvement | Illustrative owner expectation | What must be established |
| --- | --- | --- |
| Lower resource use | Three low-effort agents achieve the accepted result of one xhigh agent | Comparable acceptance, with lower complete measured resource use including coordination, failed attempts and verification |
| Higher completion probability | Several approaches solve a hard task that one approach often fails | More independently accepted outcomes under a comparable total resource allowance; distinguish collaboration from repeated sampling |
| Shorter elapsed time | Three agents complete in one day a task one agent cannot complete within three days | The same acceptance conditions, measured elapsed time and complete resource use; additional parallel resources must be disclosed |
| Less human work | The result needs fewer corrections, reminders, manual checks or recovery steps | Reduced human intervention without hiding unresolved defects or relaxing requirements |
| Improvement across tasks | Later relevant tasks reuse verified knowledge and successful methods | Lower cumulative effort or better accepted outcomes on subsequent tasks, including retrieval, maintenance and correction costs |

These are alternative or combined benefits, not a promise to improve every dimension on every task. For each scenario, select one primary benefit and limits on the others before judging success. A faster run that costs substantially more can still be useful when speed is the user's priority; it is not evidence of resource savings.

## Why collaboration might help

Independent parts of a task can be executed concurrently. Complementary capabilities can cover different sources, formats or methods. Several bounded approaches can search different hypotheses, and independent checks can detect errors before final acceptance. Shared evidence can prevent repeated mistakes and allow one participant's finding to improve another participant's work.

These mechanisms have limits. A sequential dependency chain restricts useful concurrency. Participants can repeat the same errors or converge prematurely after reading each other's claims. More conversation can consume the resources that were meant for solving and checking. No arithmetic equates three low settings with one xhigh setting: native effort names and meanings differ, and effort is not total resource use.

The current evidence does not establish an ymp advantage. External controlled work reports that relative multi-agent performance depends on task structure and coordination; that motivates conditional policies, not a universal team-size rule. See [Kim et al., Towards a Science of Scaling Agent Systems, v3](https://arxiv.org/abs/2512.08296v3). The proposed ymp mechanisms and success criteria are design inferences, not results demonstrated by that paper.

## What the comparison must include

1. **One strong native agent with the full allowance.** It keeps its normal tools, planning and correction capabilities. Do not compare a single artificially restricted answer with an unrestricted team.
2. **Several independent attempts within the same aggregate allowance.** Selection or verification of their outputs also consumes resources. This measures the benefit of multiple attempts before attributing it to communication.
3. **A cooperating team.** Charge planning, communication, execution, review, synthesis, native internal requests and failed attempts to the same run. State what information participants share and when.
4. **Relevant retained experience.** For repeated-task comparisons, distinguish useful generalization from replaying a familiar answer. Knowledge must retain scope, provenance and a correction mechanism; stale reuse that misses changed requirements fails acceptance.

Use a result-specific acceptance standard: observable behavior for software, required contents and inspectable files for documents, grounded claims and coverage for research, or explicit constraints for plans. Preserve uncertainty where correctness cannot be determined. A model's self-confidence or team vote alone is not an independent success label.

The approved intent distinguishes acceptance from confirmation. An independently reviewed qualitative result may be accepted without confirmation, but it earns no reputation and must be reported separately from externally confirmed success. Comparative reports must not silently combine these categories into a single confirmed-quality score.

Record resource units separately. Raw tokens from different models are not automatically equal cost, and native subscriptions may not yield an authoritative currency charge. Preserve usage coverage and unavailable values; where accounting is partial, report the comparison's limitation instead of asserting equal expenditure. Elapsed time and human effort remain separate measurements.

This comparison design does not authorize paid provider experiments. The prior [evaluation protocol](../research/experiment-protocol.md) remains a proposal and must be revised around the chosen product scenarios before use.

## Architectural consequences to evaluate

- The system needs a bounded way to choose execution width, model and effort together. An Oracle would be one optional source of advice in that process, charged to the same budget.
- Useful concurrent work must be possible when dependencies and resources permit it. The current take(1) ready-task limit prevents this; YMP-115 records the implementation gap.
- Starting a session must not require a consultation with every available agent. Activation, context construction and native internal loops all need explicit resource control.
- Shared evidence and independently checked results must survive an individual failure. Reassignment must preserve completed work, total spend and unresolved side-effect uncertainty.
- Verified knowledge and execution observations must remain usable across providers and later sessions, with applicability and correction. More stored messages alone do not establish improvement.

The runtime's authority and the temporary scope of agent roles are fixed by the approved intent. Methods using a temporary planner, peer proposals or several independent approaches are assessed within that boundary. Representative workflows, executable-contract design and implementation order are now recorded in the [delivery plan](../tasks/plan.md).

Tracked in [YMP-010](../tasks/README.md#ymp-010). The [research findings](../research/research-program-findings.md) remain historical evidence with stated limits.
