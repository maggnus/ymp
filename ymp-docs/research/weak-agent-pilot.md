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

The owner withdrew the Sonnet substitution on 2026-09-14 and restored the accepted
original pair: gpt-6-astra low for the proposed strong solo baseline and
gpt-5.6-luna low for weak participants. Both use the accepted Codex boundary.
The exact accepted d12d40a source and original phase manifests remain applicable.
This restores the model choice and prior proposal, not spending authorization.
Native model strength remains a hypothesis to check through the weak-solo and
strong-solo outcomes; no measured comparative result exists yet.

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
The third participant must use the weak model too. In the current ymp condition,
the reserved final reviewer cannot also produce the artifact: integration remains
with an eligible producer. The first pilot records the existing Engine behavior;
it does not claim a new coordination protocol has already been implemented.
No strong agent may solve,
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

## Native runner readiness review

Read-only reviewer: `52111c72-73e2-4ddc-b677-5573306ba1a9`, 2026-09-14.
The current main source supports preparing this pilot without YMP-148, but not
all six measured conditions are runnable with complete accounting yet.

- Solo and isolated attempts can reuse `ymp_providers::run_turn`. The existing
  CLI `ask` consumer drops usage and does not apply session resource admission;
  a thin evaluation adapter must retain events and enforce the aggregate rules.
- Cooperation uses the current Engine with two or three genuine participants
  configured for the discovered weak model. Separate native contexts are allowed
  and must retain their own IDs and origins. No renamed self-review is allowed.
- Engine solo cannot omit independent review. In a two-member ymp team one member
  is reserved for final review; at most one can produce. In a three-member team,
  at most two can produce. Count reviewers within the declared team and allowance.
- The existing eval driver is scripted and computes artifacts programmatically.
  Its successful runs demonstrate contracts, not model performance. Retain that
  distinction when adding a native mode to the evaluation package.
- Product defaults are not an experiment budget. The runner needs an explicit
  overall deadline, invocation limit, observed-token admission/stop threshold,
  cancellation and unknown-usage rule. Hard in-flight token enforcement has not
  been demonstrated. Cache is included in input; reasoning is included in output.
- Native internal delegation and actual effort must be controlled and observed
  before a one/two/three-participant claim. `--no-adaptive` disables reputation
  influence, not the current allocation algorithm. Shared write access may force
  serialization; do not claim parallel execution merely from team size.

The preparation fork may add the minimum native adapter inside the existing
ymp-eval-driver package, reusing its existing dependencies. No production policy,
authority, provider, CLI or UI change is authorized by this preparation. Its
native calls must be tested with scripted/protocol fixtures only until the final
concrete quota and execution settings are approved. A separate evaluation binary
is a development tool, not an extra distributed application dependency.

## Preparation candidate and native follow-through

Candidate `15529e6bba9995964507d8ba7c28c13870968382` provides separate fixture
variants, discriminating validators, public-only selection, submission freezing,
and 12 offline protocol cases. Author checks report 36 Python tests, 580 Rust
tests with two ignored, formatting and strict Clippy passed. These are author
results for preparation, not independent acceptance or model measurements.
The native command still unconditionally refuses before spawning a provider;
this is not a complete measurement consumer and YMP-201 remains in progress.

Continue in the same fork by binding the frozen manifest, discovered models and
fixed settings, visible inputs, native observation, aggregate admission, ordinary
Engine execution, public selection and final export. Exercise this complete
consumer with controlled protocol fixtures before requesting the remaining
concrete owner quota approval. Do not replace the unconditional refusal with an
unverified bypass.

The [official Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)
documents `features.multi_agent`, `features.memories`, memory generation/use
switches, named filesystem permissions and transport retry settings. The
[advanced configuration guide](https://learn.chatgpt.com/docs/config-file/config-advanced)
documents one-run CLI overrides. Existing ProviderConfig.args is a possible
integration path. Installed-version support and effective behavior must still be
verified without inference; absence of these fields on TurnRequest alone is not
proof that native configuration is impossible.

Scientific requirements distinguish additional participants from transport
retries or multiple requests in one participant's ordinary tool loop. The latter
may remain if fixed, recorded, included in accounting and bounded by the stated
stop rules. Hidden extra participants, unaccounted work and exposure of private
answers invalidate the corresponding interpretation. An observed-token stop
threshold with disclosed in-flight overshoot is an acceptable type of proposed
resource envelope; a hard billed-token ceiling is not claimed. The owner has not
yet approved particular values. Synthetic 12,000-token/120-second limits do not
establish that the real orchestration has enough allowance to be meaningfully
compared. Prepare a small explicit calibration allocation and a separately stated
conditional pilot allocation before approval.

## Connected candidate under independent review

Candidate 8a2acaf (consumer source 0ac1650) now implements the common manifest
consumer for all six conditions. The author reports all 12 local protocol cells,
631 Rust tests and 52 Python tests passing, with two existing ignored tests.
Installed Codex configuration and artificial access canaries were checked without
a model turn. These are preparation claims under independent review, not native
performance results. The product source matches the accepted P0 base 1c17f4e.

The concrete proposals are six calibration outcomes (80,000 observed input plus
output tokens, 12 outer invocations and 480 seconds per outcome) and twelve
conditional measured outcomes (160,000 observed tokens, 16 invocations and 900
seconds each). Their summed administrative thresholds are 480,000 and 1,920,000
observed tokens, respectively; 2,400,000 across both phases. The outer invocation
ceilings total 72 and 192, and group deadlines total 48 and 180 minutes. These are
maximum proposals, not observed consumption or approved allocations; native
in-flight overshoot and currency cost remain as stated in the manifests. The
parent has not requested owner approval yet because acceptance is still pending.

### Parent finding: distinguish task failure from measurement failure

The candidate currently requires objective_success=true for every calibration
outcome before permitting the pilot. It also stops later conditions whenever an
outcome is not interpretable; missing artifacts, a noncompleted Engine outcome or
a known limit can enter that category. The independent executable reviewer is
checking concrete instances. A false external score itself is already supported
by the Python consumer and is not the identified defect.

A healthy, fully accounted attempt can legitimately produce an incorrect answer,
a missing deliverable, an ordinary task rejection or an unfinished result within
its declared allowance. Preserve these as negative outcomes. They must not require
successful weak-agent answers to establish the measurement path, or automatically
prevent remaining matched conditions from being measured.

Calibration should establish that controls, attribution, accounting, stopping and
external scoring work. Instrument failure, unexpected extra participants, leaked
private answers, an invalid envelope, unknown spend or unconfirmed termination
remain distinct reasons to stop when continuing would violate the approved scope.
A deadline/limit with complete final accounting and verified termination is a
measured failure; a cancellation with unknown accounting still triggers the
unknown-usage stop rule. Do not solve this by relabeling all failures successful
or by ignoring shared resource restrictions. Freeze the final continuation rules
and revised manifests before requesting owner approval.

## Historical Sonnet baseline follow-up (cancelled)

The accepted d12d40a runner was bound to Codex for every condition. The same author
now prepares a narrow provider-aware strong-solo path using the existing Claude
backend and SDK bridge. Preserve all accepted outcome, deadline, candidate-access
and one-phase-approval controls. Do not relabel Sonnet as a Codex model or create
a parallel production provider. Weak conditions stay on their accepted Codex path.

Verify Claude-native context, tools, memory/subagent suppression, effective low,
access restrictions and canonical usage without model inference. Reuse existing
normalizers rather than assuming identical wire cache/input semantics. Fresh
protocol fixtures must reach the same selection/seal/score consumer. Freeze new
source, provider controls, executable and manifests for independent acceptance.
Existing numerical limits remain proposals without enlargement; any changed
resource interpretation must be stated before the next owner decision.

The earlier approval question is superseded by this model correction. No consent
to spend on either the Astra or Sonnet proposal is inferred from it, and no real
phase, calibration or model-quality result has been produced.

## Restoration of the accepted Astra proposal

The owner's subsequent instruction was to return to the original configuration.
The Sonnet adaptation was cancelled and the author was instructed to stop related
subtasks, preserve any work reversibly, and leave the accepted d12d40a checkout
intact. The parent observed a clean checkout at d12d40a and verified both original
manifest hashes plus the retained accepted runner. A delayed Sonnet task notification
does not override the newer owner instruction.

The original exact calibration and conditional-pilot proposal was restored at
that point, with its previous limits unchanged. The owner subsequently prohibited
all Astra use until an explicit future request. The proposal is now inactive and
cannot be executed. No substitute strong model is selected, and no spending is
authorized.
