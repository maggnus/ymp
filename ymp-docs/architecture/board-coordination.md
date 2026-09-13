# Shared board commitments and revisions

YMP-112 adds durable coordination to the existing runtime, SQLite metadata and decision journal. Agent text remains a proposal. Runtime validation alone changes pending work, current responsibilities or membership. Initial plans still receive independent review; every candidate result still uses the existing independent acceptance and confirmation contracts.

## Public interfaces and work flow

`BoardProposalPolicy` is an injectable typed strategy. It receives a `BoardSnapshot` and selects existing pending proposal IDs. The default `OrderedBoardPolicy` processes proposals in submission order. `Engine::with_board_proposal_policy` can replace this choice without providing mutable storage or authority to the strategy. Allocation templates use the separately injectable allocation policy; the runtime validates the final board-selected configuration.

An admitted participant reads `board_read`, then calls `task_propose` with the returned `plan_version`, a rationale, and a typed `change`. The runtime binds the proposal to the actual session, agent, assignment, invocation and grant. Task changes also name the exact `{task_id, version}` read from the board. `team_post` continues to support team-wide and addressed messages; historical participants and messages remain available. Legacy `task_propose` calls containing only `title` and `description` retain their chat-suggestion behavior. Supplying those fields with `plan_version` creates a durable new-task proposal.

| Change | Effect after validation |
| --- | --- |
| `accept_responsibility` | Record the actual proposing agent as responsible for the next attempt, with explicit model/effort settings |
| `assign` | Distribute or explicitly reassign pending work to a permitted agent and configuration |
| `revise` | Append an approach and additional dependencies/checks to a ready task; preserve an existing current responsibility |
| `add_task` | Add a bounded, independently checked subtask using stable dependency IDs |
| `membership` | Change current participants within captured restrictions, retaining historical profiles and contributions |

The normal execution loop consumes proposals at work-wave boundaries. `Engine::commit_board_proposals` exposes the same consumer to trusted clients. It returns without changing proposals while native assignments are active. A committed responsibility does not issue a capability, start native work, spend a new allowance or establish acceptance. The scheduler honors that responsibility, performs an atomic current-version task claim, and then uses the existing budget, workspace and assignment-grant admission path. If the responsible agent has already been selected in the current wave, the task waits with its commitment intact. Earlier claims still execute and receive review if selecting later work finds a real constraint error. Actual invocation records preserve requested, sent and reported settings.

## Versions, atomicity and constraints

The plan digest covers task IDs and definitions, excluding lifecycle and workspace-location changes. A task version covers its complete current record and the last committed responsibility generation. Actual membership, eligible identities and the reserved independent reviewer have a separate digest; reordering identical membership does not make a proposal stale. Task claims compare the current version, state, executor availability, membership and reserved-review obligations in the SQLite transaction.

The proposal decision, task changes, responsibility and any membership/allocation changes commit together. Competing claims have one winner. Revisions and membership proposals cannot overwrite changed versions. Grant validation, request replay protection and proposal persistence share the team-call transaction; expired or foreign capabilities cannot submit changes. Live capability secrets remain process-owned and absent from board records.

A revision cannot rename or delete existing tasks, replace their objectives, remove dependencies/checks, enlarge task access, change trusted acceptance contracts or rewrite accepted results. It may add an approach, obligations or new work. Graph validation rejects unknown dependencies and cycles and retains the existing 24-task ceiling. Board input is limited to 256 proposals per session, 32,000 bytes per change and 4,000 bytes per rationale. These are bounded first-release defaults, not claims about optimal coordination.

Membership and settings choices are checked against captured user restrictions, current eligibility, supported native controls and preserved independent review. Pending responsibilities prevent silent participant removal; explicit reassignment ends the prior responsibility without deleting its history. Work and resources are rechecked before commitment, and native invocation admission still enforces the shared budget and verification reserve. Coordination never resets spend or reserves a second copy of remaining resources. Unknown native usage retains its existing limitations.

## Failure and recovery

An admitted execution with uncertain effects remains unavailable for revision or reassignment until the existing runtime recovery path captures its current artifacts and admits fresh independent inspection. A selection that ended before native admission has no execution effects to inspect. Completed sibling tasks and their precise result/confirmation records survive. Failed or interrupted source assignments cannot turn their outstanding proposals into committed changes.

Failure does not automatically increase effort, expand membership, replay native work or grant a standing role. The next admitted attempt receives fresh settings validation, workspace access, budget admission and a new capability. Independently accepted but unconfirmed results continue to earn no reputation.

## Shared view contract

`Store::board` and `Engine::board` expose `BoardSnapshot`: session and plan version, versioned tasks with any current `BoardCommitment`, durable proposals with pending/committed/rejected status, and current `TeamState`. A commitment identifies its proposal, task, responsible agent and model/effort settings. Each `DecisionRecord.links.board` contains the strategy identity, original bound proposal, decision and rationale, any commitment and resulting plan version. This contains audit IDs, never capability secrets.

Existing task and team views should show the responsible agent/settings, proposal disposition and reason, relevant versions and current/historical membership alongside existing acceptance/confirmation grades. Runtime task updates and board-decision messages use the existing event channel. Corresponding TUI changes belong to the separately delegated Claude Opus 5 implementation (high thinking for new assignments); this backend artifact does not claim UI acceptance.

## Executable evidence

[Board coordination tests](../../ymp-rust/crates/ymp-runtime/tests/board_coordination.rs) exercise the public engine and live local team socket with scripted native backends and low effort. They cover useful proposal consumption, two conflicting agent claims, different injected strategy ordering, stale task/plan/membership versions, distribution, reassignment, approach revision retaining responsibility, new-subtask acceptance, malicious settings/identity/contract fields, expired capabilities, simultaneous atomic claims, and inspection before recovery replay while preserving a confirmed sibling.

[Negative controls](../evidence/ymp-112/negative-controls.txt) record required failures after disabling the engine board consumer and after removing the responsibility generation from task versioning. Both mutations were restored. The [validation record](../evidence/ymp-112/validation.md) records the final formatting, clippy and workspace test results. All provider work is mock/scripted; no real inference or credentials are used.
