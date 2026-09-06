# Battleship: the complete operator journey

**Closed by owner request.** Development of this subproject is stopped. The source and artifacts
are retained as an archived prototype, not an accepted product direction. Do not resume this work
from old goals, schedules or plans without a new explicit owner instruction.

Status: product scenario with an implemented persistent simulator, 2026-09-06. The simulator
covers empty startup, provider configuration, clarification, explicit development authorization,
agent recruitment, failure/correction, external-tool records, verification, acceptance, cancellation
and retry. Execution observations are simulated. Local entity checkpoints and JSON report exports are real;
no provider backend, generated game files, model calls or actual development processes are supplied. Interface copy is English; user text is
preserved verbatim. The remaining product requirements below are not claims of implementation.

## Product boundary

ymp helps a person specify a software task, obtain work from agents, examine evidence and accept
changes. It does not embed or host the produced game inside its own interface. Agents may use
external commands and tools, including tests, the application itself, a development server and a
browser, as part of authorized development and verification. These are observable tool operations,
not a playable screen inside ymp. The operator can also ask for an external launch in Chat.

A tool invocation identifies its agent, purpose, command/tool, working directory, process state,
output and exit result. A long-lived server remains visible with its owner and a stop action;
finishing an agent does not silently abandon an untracked process. URLs, screenshots, logs and
test artifacts are evidence or external destinations, never an embedded game view.
This scenario uses a browser-based Battleship game against a computer, delivered as source files.
Its rules below are explicit choices for this task, not claims that every Battleship variant has
the same rules. No actual model, development task, experiment or game is started by this document.

## Five primary screens

| Screen | The question it answers | Contents and connections |
| --- | --- | --- |
| `Chat` | What are we making, what needs my decision, what has changed? | Operator conversation, clarification, task summary, development controls and final handoff. Right column: relevant task, run, agents, board and provider state. |
| `Tasks` | What work did I request, and what is its outcome? | Human tasks. Enter: specification, development runs, assignments, candidates, checks and changes. A human task is not one row per agent. |
| `Agents` | Who is working, on what authority, and what happened? | Actual registered agents for the selected task, including a model-powered clarification session. Enter: source, assignment, frozen launch configuration, attempts, messages and output. |
| `Board` | What did the agents tell each other? | Attributed messages for the selected task/run, authors, references, audience, publication age. Enter: full message and links to its author and cited messages. |
| `Providers` | Where can agents obtain model execution? | Supported providers, enabled state, runtime, observed availability, models and observation age. Enter: configuration and permitted models for future development. |

Project, task and run are context, not three more dashboards. Results, verification and attempt
logs are details of the selected task or agent. Model catalog and pool limits belong below
Providers. Conversation history belongs below Chat. Technical events belong below agent/run
details. They need not compete with the five primary screens in the main picker.

The header always identifies the current project and task. Agents and Board inherit that scope;
they do not silently mix messages from unrelated tasks. Providers is visibly global configuration.
Following a relation preserves the original table filter, sort, selected row and scroll position.
Esc returns to that exact view. Returning to Chat preserves unfinished text and its cursor.

There is no permanent top navigation row. `/` selects screens and context-appropriate actions;
Ctrl K opens the same picker without disturbing an unfinished message. Ctrl F (or `f` in a table)
edits the current screen's filter. No filter input occupies space outside editing.
An applied filter stays in the title. `?` shows context-specific help.
While typing a message or filter, punctuation remains text. A shortcut must not consume a path
or question mark from a draft. Labels such as `Agents`, `Author`, `Provider`, `Changes` and
`Checks` are actual navigable relations, not decorative prose naming unavailable destinations.

## 1. First opening

The operator opens ymp in the intended project directory. The header identifies that directory;
it does not guess a different destination. With no prior task, Chat has an empty conversation and
one message input. The right column shows `No task`, `No active agents`, and the provider state.
There are no sample agents, invented events, fake progress or pre-existing board messages.

If prior work exists, the application restores its recorded task and conversation. It checks the
runtime before describing an old process as still running. A stored `Running` label alone is not
evidence of a currently supervised process. Uncertain recovery is shown explicitly.

The operator writes:

> Хочу разработать игру «Морской бой».

This creates a task draft and stores the message. If no provider is configured, a local system
notice says `Task saved · provider required`, with the `Providers` action. Storing a message does
not launch a development agent or fabricate a model answer.

## 2. Provider setup and the source of agents

Providers lists the supported families even before configuration. A provider is an account/source
of model access, a runtime is the installed client used to reach it, a model is an available
capability, and an agent is a concrete session created for work. These are separate entities.

The operator selects a provider and sees:

- its enabled decision;
- the runtime through which ymp can use it;
- observed runtime availability and authentication origin, without exposing a secret;
- the observed model list and when that observation was made;
- launch settings actually supported by the installed profile;
- which model entries future development may use and the existing pool ceilings.

Before enabling, concise copy states that this permits the provider to receive task content.
`Enable` and `Refresh` name their own effect. Enabling may permit an explicit capability probe;
it does not create agents to fill capacity. `Not checked`, `Unavailable` and a confirmed result
are distinct. Finding a credential file is not proof that a remote provider accepts it.

For the currently pinned Codex/Claude profiles, the known model and `low` effort are displayed
as fixed profile values. The prototype must not offer unsupported effort choices merely because
those choices look useful in a form. Unsupported settings are a future implementation requirement,
not a successful configuration action.

The pool is an allowed set and a ceiling, not a team roster. Existing `max_agents` and
`max_concurrent_attempts` limits are disclosed under advanced settings. No desired team size,
required researcher/developer/reviewer roles or automatic replication is invented.

After setup, the original task and message are still present in Chat. `Continue conversation`
submits the held message. If the provider was already enabled, sending the first message performs
this step directly under the existing permission.

## 3. Who asks the clarification questions?

A model-powered reply is real work. If a model is used to clarify the request, that session is
observable and accounted for in Agents as `Task clarification`, with its provider, model, scope
and calls. It is not an invisible planner hidden behind the word “system”. Local notices about
missing configuration are distinguishable from that agent's messages.

Sending a message authorizes that conversation turn. It does not authorize source changes or a
development run. The clarification agent can discuss and propose a specification; its execution
scope does not permit development writes. While awaiting an operator answer it holds no active
model process merely to make a status look alive. Its recorded conversation can be resumed for a
later turn. Development is a separate explicit action with a separate authorization.

The assistant asks one compact set of consequential questions, offering sensible editable choices:

1. `Platform` — browser, desktop or terminal? Proposed: browser.
2. `Opponent` — computer or another person? Proposed: computer on the same device.
3. `Rules` — the proposed board, fleet, placement, turns and winning condition.

For this scenario the operator answers:

> В браузере, против компьютера. Поле 10×10. Обычные корабли, ручная и случайная
> расстановка. Без сети и регистрации. Интерфейс игры на русском.

ymp's own interface remains English. The language of the produced game is a task requirement,
not a change to ymp's interface language.

The assistant restates ambiguous rules rather than hiding them in generated code: one four-cell
ship, two three-cell ships, three two-cell ships and four one-cell ships; no touching, including
diagonally; hit retains the turn; miss transfers it; already attacked cells cannot be attacked
again; the last sunk opposing ship wins. The operator can amend any of these choices.

The resulting compact specification also states manual/random placement, board reset before
start, visible own ships and hidden opposing ships, hit/miss/sunk feedback, victory/defeat and a
new-game action. Network play, accounts and persistence are excluded for this first task. These
are explicit scope choices, not a permanent restriction on future requests.

## 4. Ready for development

Chat presents one reviewable task summary. Its details contain:

- the goal and the agreed behavior, including the exact rules;
- the destination project and its starting revision/state;
- permitted model entries and the actual resource ceilings;
- required deliverables: source files and a usage document;
- acceptance checks and what remains manual;
- the permitted external development tools; the game itself is not embedded in ymp.

The visible next action is `Start development`; adjacent actions are `Edit task` and `Providers`.
There is no generic `Run` action that might mean playing the game. Missing requirements name the
specific issue and its destination. Unknown cost is shown as unknown, not zero. If a required
resource bound is missing, the UI obtains it before starting rather than silently assuming infinity.

Changing the specification after preparation invalidates the old authorization. A user cannot
accidentally authorize one set of rules while executing a different hidden revision.

## 5. What Start development actually creates

The operator selects `Start development`. The application records a development run, freezes the
approved specification, source identity and allowed provider/model entries, and creates one
origin participant according to the existing pool rule. It does not immediately manufacture a
full team. A double activation is deduplicated rather than creating two paid runs.

An agent becomes a visible entity when its creation is recorded. A request that has not been
admitted is a pending request, not an agent pretending to run. Process launch and process
readiness are subsequent observable facts; failure during either remains visible.

The new agent receives an explicit assignment containing the agreed task, constraints, its
isolated workspace, resource allowance, communication access and output obligations. Expected
behavior is readable: work on this task, use the permitted tools, report blockers, attribute
messages, and return a candidate or a reason it could not return one. There is no promise that a
role name guarantees competent behavior.

The agent's details answer, without visiting logs:

- Why was it created? `Started by operator · task T-001 · run R-001`.
- What is it supposed to deliver? The assignment and acceptance criteria.
- What does it use? Provider, runtime, model, effort and the frozen launch settings.
- What can it change? Its workspace and explicit capability scope.
- What is it doing? The last recorded operation and state, with its observation time.
- What has it said or returned? Board messages and candidate references.

Opening Providers and disabling an account changes future eligibility. It does not rewrite the
recorded configuration of an existing agent or claim to have stopped its process. Stopping work
is a separate task/agent action with an observable completion.

## 6. A concrete development trace

The following is an illustrative trace, not a mandatory team composition or evidence of model
capability. The product must also support a task completed by one development agent.

1. Agent A-002 starts the development assignment for R-001. It reads the specification and
   constructs a candidate in its own workspace. A-001's clarification work remains attributable,
   but does not count as a concurrently running development process after its turn ended.
2. A-002 publishes a message describing the proposed separation of game rules and visual input.
   Board shows the actual author, content, publication age and references. The main conversation
   receives a brief relevant summary, not every tool call.
3. If A-002 requests help checking placement rules, the request names its scope and resources.
   Only after admission does A-003 appear. Its creation source links to A-002's request and its
   assignment. A-003 is not present merely because a settings panel allowed three concurrent
   processes. Rejected or queued requests show their reason without increasing the agent count.
4. A-003 examines the assigned rule behavior and publishes an observation: a placement example
   permits diagonal touching. Its message cites the relevant case. This is the agent's claim;
   it is not displayed as an independent passed/failed verification result.
5. A-002 changes its candidate and returns a new revision. A task detail shows which candidate
   was checked and whether the evidence still matches it. Old evidence does not automatically
   apply to newly changed bytes.
6. A-002 returns a candidate and finishes. `Completed` means the agent ended its assignment;
   the task still says `Verification pending` until its separate checks have an outcome.

Agent lifecycle presentation follows recorded facts: creation, start, running, yielding/waiting,
resumption and terminal outcome. A wait includes its reason when known. `Failed`, `Interrupted`
and `Completed` are not collapsed into a generic stopped state. Progress percentages and
estimated completion times are absent unless a meaningful measurable basis is available.

No inference such as “silent for 30 seconds means stuck” is introduced. The UI can report time
since the last observed event, but lack of a message is not proof of a hung process.

## 7. Navigation during that trace

Chat's right column summarizes the current task and actual running/waiting/finished counts.
`Agents` follows that task's population; `Board` follows that run's messages. It does not show
512 participants from a stress fixture in an ordinary first run.

In Agents, the practical columns are agent, state, model, effort, AGE and assignment. The
provider, exact attempt, workspace, resources and creation source are in the row's detail.
Selecting `Task` there opens its assignment in context; `Provider` opens the actual frozen
route alongside current provider configuration; `Messages` filters Board to the author.

In Board, message, author, AGE and a concise content excerpt are the useful list fields. Enter
opens the full attributed message and its references. Selecting the author returns to the
corresponding agent. Quoted content is inert, even if it resembles a slash command or terminal
escape. Board is agent communication, not a second operator chat and not a “verified knowledge”
database. Long-term knowledge/transfer needs its own later evidence; this scenario makes no such
claim. Operator decisions continue through Chat so an arbitrary board post cannot authorize work.

In Tasks, one row is the requested Battleship game. Its detail groups specification, runs,
assignments and results. Hundreds of agent assignments must not turn the operator's task list
into hundreds of unrelated projects.

## 8. AGE and timestamps

| Entity | Useful time display | Meaning |
| --- | --- | --- |
| Human task | `AGE` | Since the task was recorded, including time awaiting clarification; not time spent computing. |
| Development run | `DURATION` in task detail | Since actual run start; stops at the terminal event. |
| Agent | `AGE` | Since that participant's recorded creation. Attempt duration and last activity are separate details. |
| Board message | `AGE` | Since publication; opening or receiving the message does not reset it. |
| Provider | `CHECKED` | Age of the last real observation, not age of the provider or account. |
| Model/pool settings | No generic AGE | Availability/provenance matters; a manufactured creation age provides no useful fact. |
| Candidate/check | Recorded creation/check time | Establishes which result and evidence belong together; a green age is not a validity guarantee. |

Missing, future-dated or incompatible timestamps render `—` with a specific explanation in
details. Event sequence numbers and UI tick counts are never converted into elapsed time.
Age survives restart by using recorded timestamps. Completed agent AGE can keep increasing;
its execution duration does not. The interface must not silently change the meaning at completion.

The current backend already has `MessageRecord.published_at` and provider `observed_at_ms`.
Participant/task-contract records inspected for this prototype lack the corresponding durable
creation timestamps. Their AGE requires a real projection extension; until then it is unknown.
This is an explicit implementation gap, not permission to fill cells with plausible demo values.

## 9. Work stops or needs a decision

The main conversation surfaces consequential events with one appropriate destination:

| Event | What the operator sees | Allowed continuation |
| --- | --- | --- |
| No usable provider | Exact observed problem and `Providers` | Configure or select another permitted route; no ghost running agent. |
| A required answer is missing | The question and what waits for it | Answer or revise the task; unrelated work can continue if authorized. |
| An agent process fails | Failure reason, affected assignment and retained output | Inspect; an explicit retry is a new attempt with provenance, not erased history. |
| Recruitment exceeds a ceiling | Pending/rejected request, capacity/resource reason | Wait, decline or explicitly revise an applicable bound; no hidden overspend. |
| A budget/time allowance is exhausted | Actual exhausted allowance and partial result | Review the partial result or explicitly authorize further work. |
| Verification fails | Exact candidate, failed check and evidence | Authorize/use remaining correction work; never label the task complete. |
| Verification infrastructure fails | `Not verified` and infrastructure reason | Repair/retry the check; do not misclassify it as failed game behavior. |
| Operator stops development | `Stopping` until termination is observed | Preserve conversation, assignments and candidates; only then show terminal state. |
| ymp restarts | Restored history and reconciled process state | Resume supported work, start a new attempt or inspect partial output; never relabel an unobserved process as running. |

Edits during development are recorded as a proposed task revision. They do not silently alter
the current agents' frozen assignment. The UI states which work continues under the old version
and which new authorization is needed.

## 10. Verification through external tools

The frozen acceptance plan can check source/build outputs, isolated game-rule tests and actual
application behavior through external tools. Concrete rule tests cover board bounds, the selected fleet,
overlap/touching, legal placement, repeated shots, hit/miss/sunk, turn changes and the terminal
condition. A check is tied to the candidate it examined and retains its actual result/output.

For this browser game, an agent may start an external development server, wait for its actual
readiness, and use a browser tool to exercise placement, shots, turn changes and the terminal
screen. Agents inspect console errors and retain screenshots or other observations when useful.
The server, its address, logs and lifetime are visible in agent/run details. Stop it when the check
ends unless an explicit continuing purpose exists. A successful process start alone is not proof
that the server is ready, and passing one browser scenario does not verify every possible game.

If a browser tool, display or other dependency is unavailable, the affected checks remain explicitly
unverified. The report states what was really exercised, what failed and what remains uncertain;
it cannot claim a full test merely because an agent said the result worked.
The verification plan itself is reviewable before development. The producing agent cannot
silently weaken its accepted criteria to make its candidate pass. A changed rule or changed check
is a visible revision rather than inherited approval.

## 11. Delivery and acceptance

Chat reports `Candidate ready for review`, followed by a compact factual handoff:

- what was implemented according to the candidate;
- where its files are and how to inspect the changes;
- which exact checks passed, failed or were not performed;
- known limitations;
- a usage document with instructions for running outside ymp.

Available actions are `Changes`, `Files`, `Checks`, `Accept changes` and `Request changes`.
They inspect or deliver the candidate rather than embedding the game. An external launch can
be requested through Chat and is recorded as a tool operation. A candidate is not automatically
applied to the operator's source tree.
Before accepting, the application checks that the destination still matches the reviewed base;
conflicting operator edits are shown, not overwritten.

Acceptance applies the reviewed changes only to their stated destination. `Applied` is shown
only after that operation actually succeeds. If the person prefers to inspect/export files first,
that outcome remains distinct from application. No automatic commit, push, deployment, browser
launch or publication is implied by accepting changes.

The user may later run the game independently. If they return and write “при повторном выстреле
теряется ход”, ymp records a revision/follow-up linked to the delivered result and repeats the
bounded development/verification cycle. Its agents can reproduce the report with external development/browser tools and retain the
observed evidence. The game is not embedded in the ymp interface.

## 12. What completion means

For this scenario, successful delivery means the agreed source and usage documentation exist,
the reported checks actually refer to those bytes, remaining uncertainty is visible, and the
operator's selected delivery/apply action completed. A run with no acceptable candidate is an
honest failed or incomplete outcome. Every agent and attempt retains its terminal record.

Chat ends with the delivered result, destination, check summary and `New task`. Task history keeps
the specification, approvals, agent origins/settings, communication, changes and evidence. No
background agent remains running merely because the conversation window is open. Closing ymp
must not make a promise about process termination that it has not confirmed.

This is completion of a development task, not proof of cumulative knowledge or a completed
research POC. Board messages and one successful game do not establish cross-task transfer.

## Original implementation sequence and acceptance

Implement this path before adding more top-level screens:

1. Empty startup; recorded request; provider setup; conversation work visible when a model is used.
2. Editable specification; explicit Start development; one attributable origin; exact assignment.
3. Agents/Tasks/Board relation navigation, real state transitions, retained settings and output.
4. Candidate/check/acceptance separation; observable external tool/process operations; no embedded game screen.
5. Recovery, cancellation, refusal and no-result states; then a separately selected large-table
   fixture for hundreds of agents.

Acceptance must exercise both the ordinary route and refusals: no provider, no authorization,
double start, failed launch, unchanged draft after navigation, rejected recruitment, unknown AGE,
changed settings during a run, failed/invalid check and a conflicting apply destination.
Keyboard help describes only existing actions. Demo operation is labeled; simulated success is
never presented as a real model call, authentication check, file application or verification.

The visual balance is moderate: one-row table records, space between header/filter/table and
between semantic groups, a compact unboxed composer, and no permanent empty filter field.
Conversation messages get breathing room; tables do not get a blank row after every agent.

## Source boundaries consulted

- `ymp-runtime-registry/src/provider.rs`: provider enabled/observed distinction, authentication
  origin, observed timestamp, provider → engine relationship.
- `ymp-runtime-registry/src/pool.rs`: permitted entries, max agents/concurrency, no desired team.
- `ymp-domain/src/pool.rs`: frozen provider/engine/model route.
- `ymp-domain/src/participant.rs`: start record, running/yielded/finished, terminal outcomes.
- `ymp-domain/src/commitment/records.rs`: assignments, attempts and independent return outcomes.
- `ymp-board/src/records.rs` and `observatory.rs`: publication time and attributed untrusted messages.
- `ymp-runtime-codex/src/lib.rs` and `ymp-runtime-claude/src/lib.rs`: currently pinned models/low effort.

The human-task composition, model-powered clarification visibility, full relation navigation,
timestamp gaps and recovery/acceptance surfaces above are requirements to implement or confirm.
They are not inferred to be present merely because their underlying records exist.
