# Team size and reasoning-effort policy

Status: product direction documented at the owner's request on 2026-09-12. Bounded dynamic teams are the default direction; fixed user constraints remain supported. This is a design contract, not implemented behavior or evidence of an optimal allocation policy. Product implementation remains paused during YMP-010. The coordination topology and numerical defaults are still to be selected.

This document expands the short principles in [intent.md](../../intent.md). Its purpose is to support the [product goals](../product/requirements.md#goals): accepted results within resource and time constraints, less human intervention and useful accumulated experience.

## Pool, membership and active work

| Concept | Meaning | Consequence |
| --- | --- | --- |
| Available pool | Identified agents and native capabilities eligible for selection | Discovering twenty available agents does not require twenty model calls |
| Session team | Current participants assigned responsibilities in a session | Membership may change within constraints; past participants and their contributions remain in history |
| Active invocations | Work actually executing now | Concurrency depends on ready work, occupied agents, dependencies, shared resources and provider limits |
| Assignment configuration | Agent, native model and reasoning settings chosen for particular work | Effort can differ between assignments of the same agent |

A participant waiting for a dependency is still a member. Finishing an invocation does not implicitly remove the participant, and removing a participant does not erase its history. Pool size, current membership, cumulative historical membership and concurrent invocations must not share one ambiguous counter.

Independent assignments may run concurrently when their resource constraints permit it. Twenty agents with one useful ready assignment do not justify nineteen speculative calls. Several useful ready assignments should not be serialized simply because the initial plan had one coordinator. Evidence collection, alternative approaches and independent verification can supply useful parallel work, but each needs a defined contribution and allowance.

## Dynamic and fixed constraints

The default policy may expand, replace or reduce current membership within configured limits. Changes happen at useful work boundaries and preserve ongoing responsibilities. A departing participant's unfinished assignment must be completed, explicitly handed over or stopped with its state recorded; departure is not an excuse to forget it.

Fixed settings constrain this same policy rather than introducing a second controller:

- **Fixed size:** the number of current participants is pinned. Their identities may be selected or replaced unless separately pinned. A fixed size does not require every participant to execute continuously.
- **Fixed roster:** the current participant identities are pinned. Their assignments and active concurrency may still vary. This is stricter than restricting the eligible pool to an allowed list. No additional participant is silently added for review, recovery or consultation.
- **Fixed model or effort:** the corresponding allowed set contains one value. Other choices may remain adaptive. Model-dependent effort support still has to be validated.
- **Bounded dynamic operation:** the policy selects among eligible participants and settings up to the recorded limits. A membership ceiling and an active-invocation ceiling have different meanings.

Inconsistent constraints are reported explicitly. A fixed size must agree with a pinned roster and cannot exceed the eligible pool. A required independent reviewer cannot be replaced by self-acceptance just because all permitted participants produced the result. Replacing a failed participant may satisfy fixed size while violating a fixed roster; the policy must distinguish the two. Constraints never force filler work merely to keep everyone occupied.

## One authority model

| Responsibility | Owner |
| --- | --- |
| Goal, deadline, resource limits and explicit pins | User constraints, supplemented by recorded defaults |
| Proposed work, approach, required capabilities and supporting findings | Agents acting on their current assignments |
| Selection of method, membership, executors, models and effort | YMP's planning policy, using proposals and verified experience |
| Validation, budget reservation, admission and durable state transitions | Runtime |
| Acceptance of produced results | The applicable independent verification process |

The planning policy is one logical decision mechanism. Whether it uses a temporary planner, peer proposals or another coordinator remains an architectural choice; no provider name grants control. An agent recommendation cannot directly expand a budget, bypass a pin or accept its own output. Runtime enforcement does not itself require an LLM call.

Configured defaults supply missing values and never silently override an explicit constraint or valid assignment decision. Record requested settings, what the adapter sent or acknowledged, and what the provider reports separately. An inherited default can remain unresolved. Effort is a native reasoning control, not a common numerical currency or a guaranteed token allowance.

## Joint allocation

At a decision boundary, consider alternative uses of the remaining resources together:

| Evidence or need | Candidate response |
| --- | --- |
| Independent ready work | Admit another executor if concurrency and budget permit |
| A missing capability or evidence source | Select a suitable participant, model or tool; obtain the missing evidence |
| A difficult reasoning step | Consider a stronger model or higher supported effort for that assignment |
| A plausible competing approach | Allocate a bounded independent attempt before sharing conclusions when useful |
| An uncertain candidate result | Allocate appropriate independent verification |
| An exhausted or unsuitable method | Revise the approach, reassign or stop with preserved progress |
| No useful ready work | Wait for a dependency or finish; do not generate activity to improve utilization |

Adding participants, raising effort and changing models are alternative resource decisions. They are not an automatic sequence of increasing intensity. Each change records what it is expected to help, which evidence motivated it, its resource allowance and how the result will be assessed. The first version may use explicit heuristics; it must not invent calibrated success probabilities or claim optimal choices.

Effort changes normally apply to the next invocation for an assignment. Do not assume a provider supports changing reasoning settings halfway through a native turn. Unchanged work need not be restarted to apply a new preference, and unrelated assignments need not inherit the change.

## Startup and adaptation boundaries

Opening the TUI, reading session metadata or inspecting the pool requires no inference. On a new task, YMP admits a bounded initial planning or execution assignment using available capability metadata and relevant verified experience. That initial work may propose a team and method; it does not grant its executor permanent authority. The bootstrap-selection rule is a remaining design choice. Do not prescribe one executor or three low-effort agents for every task.

Initial context and work are limited, and resources for required verification are protected before wider execution begins. There is no mandatory all-pool proposal or bidding round. Numerical startup allowances depend on supported native controls and must be stated honestly; they are not established by the current research.

Reconsider the allocation when work produces a result, a check fails, a meaningful new task or dependency appears, a participant becomes unavailable, or the goal or available resources change. Do not run a model to reassess the team on every token event or while nothing has changed. Repeated consultation without new evidence or an admitted allowance is not progress.

## Failure and budget handling

Failure prompts diagnosis, not an unconditional effort increment. Infrastructure errors need infrastructure handling; missing context needs relevant evidence; a flawed approach may need revision; a verified reasoning deficiency may justify higher effort. Reassignment preserves consumed resources, attempt history, completed work and any uncertainty about side effects.

All planning, discussion, execution, failed attempts, verification, synthesis and Oracle consultations count toward the session's resource use. Admission considers spend, concurrent reservations and protected verification/reporting resources. Reservations prevent simultaneous assignments from each claiming the same remaining allowance. A participant leaving does not refund already consumed resources, and retries do not create a new budget.

An ymp invocation can include multiple native model requests and tool calls. Bound native loops, context, output where supported, elapsed time and concurrency as well as outer invocations. Document which limits the adapter enforces and which depend on estimates or incomplete reporting. An admission ledger alone cannot guarantee a hard token ceiling when a native runtime cannot bound or fully report consumption. Preserve unknown usage, retain overshoot evidence and prevent further admission when the policy no longer permits it.

Keep the verification reserve usable: required reviewers must remain eligible and any pinned roster must permit their work. If constraints make accepted completion infeasible, preserve verified progress and report the unmet requirement rather than silently expanding the team or declaring an unchecked result complete.

## Experience and the optional Oracle

Retain the task context, chosen configuration, actual resource use and coverage, elapsed time, verification outcome and relevant failure causes. Evaluate configurations and methods using this evidence, while attributing work to stable agent IDs. Do not treat a higher message count, self-confidence or agreement as improved competence. Preserve execution variants rather than silently pooling unknown, low and high effort results.

Useful experience can inform when to use more participants, spend more reasoning effort or reuse a proven procedure. Changed requirements and stale knowledge still require applicability checks. A later task must not repeat a prior result merely because its prompt resembles an earlier one.

An external Oracle remains an optional proposal described in [ADR 0002](../adr/0002-agent-identity-and-reasoning.md#open-option-external-oracle). It can advise this same selection process, but cannot override constraints, become a free extra participant or replace independent verification. The roster and concurrency rules must state whether such a consultation is permitted before admitting it.

## Examples and delivery

A task with three independent research questions may begin with three bounded research assignments. Once their findings are available, synthesis and verification may need fewer concurrent invocations. If one question remains unresolved, the policy may select another approach or change its model/effort without changing every participant's settings. This is an illustration, not a fixed phase sequence or a performance result.

With three pinned participants, the same task stays within those identities. Required review must be assigned to an eligible participant that did not produce the result being reviewed. A disabled pinned participant or unavailable model is a constraint issue; it does not authorize an undeclared replacement.

YMP-109 covers pool and membership identity; YMP-110 covers selection under dynamic or fixed constraints; YMP-111 covers assignment-level native controls; YMP-102 covers admission and budget limitations; YMP-112 covers revision and reassignment; YMP-115 covers useful concurrent execution. YMP-011 records this documentation change. Offline checks must distinguish fixed size from a pinned roster, demonstrate useful overlap and conflict serialization, and reject overspending or unsupported changes. Real-provider performance comparisons remain separate and require their own authorized budget.
