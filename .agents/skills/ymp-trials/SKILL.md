---
name: ymp-trials
description: Plan, conduct and assess ymp tests, integration checks, native runs and product research using one intent-based evaluation and acceptance protocol. Use for test planning, run observation, regression or policy evaluation, experience studies and interpretation of results; scale evidence to the requested scope.
---

# Ymp system evaluation

Produce an evidence-backed product or engineering decision, not a transcript of
tool activity. This skill applies across task domains and test levels. A game,
document, data transformation, CLI check or policy experiment is a case instance.

## Load the shared contract

Read the repository's `AGENTS.md` and [intent.md](../../../intent.md). Then read
the [system evaluation protocol](../../../ymp-docs/guides/session-trial-and-acceptance.md).
Its coverage map is the single source for system dimensions and its criterion
record is the common unit of planning, observation and acceptance. Use the
[plan/report template](assets/evaluation-plan.md) for a new evaluation or update
the existing case document; do not create separate competing acceptance criteria.

## Choose the scope before tools

Identify whether the request is a focused regression, integration/release check,
diagnostic run, controlled comparison, longitudinal experience study, UX review
or reliability investigation. Select the primary product/engineering question.
Separately establish the permitted phase: planning, read-only appraisal of
retained evidence, local tests, or native/external execution. A comparison
question does not itself authorize a comparison run. For retained evidence,
first determine which question each test or observation can actually answer.
Review the coverage map and record what is tested, indirectly observed, missing
or outside scope. A broad system review must explain all dimensions; a narrow
test can reference unaffected dimensions collectively with a specific reason.

Choose the smallest sufficient evidence for that scope. A focused unit failure
does not require native inference, a full game, publication or a benchmark.
An ordinary native run does not demonstrate superiority over a solo agent.
Required repository checks remain applicable; do not add repeated full suites,
unrelated mutation matrices or new runners without an identified gap.

## Plan and execute against one criterion set

Before execution fill the question, expected behavior, counter-explanation,
evidence method, required versus exploratory criteria and success definition.
Keep fixed conditions, changed factors, models, IDs, budget, environment and
publication targets in the case document. Never inherit them from an example.
Preserve explicit user choices and existing authorization. Test planning alone
does not authorize a native experiment or external publication.

Use installed native identities/settings as actually observed. Preserve captured
policies and distinguish requested, sent and reported values. Separate observer,
implementation/review forks, runtime team and external validator roles; observer
analysis must not silently solve the measured task.

Use existing tests, runtime consumers, traces and domain validators. Before a
repeat inventory prior reachable artifacts and preserve baseline evidence
durably. Record any reversible cleanup or intervention. A fresh directory or
new agent ID does not prove isolated information or independent execution.

During execution collect meaningful transitions, failures, decisions, artifact
and evidence versions, per-agent resources and intervention records. Report new
findings with their basis. Distinguish an active native call, a real wait, repeated
unchanged work, a dead process and missing observation; elapsed silence alone is
not a diagnosis. Follow the declared stop condition without inventing ceilings.

## Analyze and accept

For material observations trace requirement -> proposal -> decision -> admitted
action -> evidence -> outcome. Find the first divergence and a competing
explanation. Distinguish protocol compliance from policy effectiveness and
unchanged output from verification that adds useful assurance.

Assess the five intent goals explicitly: quality, resources, time/useful
concurrency, human intervention and improvement from verified experience. Mark
unsupported comparisons or absent knowledge history as untested, not successful.
Do not convert cache/cumulative counts into invented cost, knowledge hits into
learning, message volume into cooperation or parallel calls into speedup.

Fill the original criterion rows with pass/fail/unknown/not-exercised and exact
evidence. Keep artifact quality, native completion, trusted confirmation, protocol
integrity and product usefulness separate. A negative test can pass by correctly
rejecting input. A blocked user task cannot be called successful because an
observer published a working file. Preserve all failed attempts and their cost.

A conditional criterion is required only when its declared trigger occurs;
missing trigger evidence is unknown, not proof that the condition was absent.

Rank proposals by user impact and evidence for their mechanism. State expected
behavior, existing subsystem, tradeoff, rejected alternative and the smallest
check that could disprove the proposed benefit. End with the decision supported
now, unresolved questions and the next justified action; do not start that action
if it is outside the current request.

Store the completed plan/report and evidence index in the permitted durable
location. When register maintenance is authorized, update
`ymp-docs/tasks/tasks.json` and generate its views through `manage.py`. Otherwise
return a proposed status note without changing the register. Improve this skill
only with reusable lessons, leaving case-specific findings in their reports.
