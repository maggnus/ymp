# Paid planning and criterion-directed work

Canonical status remains in `tasks/records/W1-0011.json`. Application exposes
the Plans consumer through the same SessionControl as intake. Named IntakePolicy,
Planner, MethodRouter and ContributionPolicy ports have concrete implementations;
their decisions retain effective parameters, inputs, basis and outcomes.

## An accountable Planner result

PlanningPrompt records the current contract, goal, criteria, method and selected
policy in the actual invocation prompt. PaidPlanningView resolves the retained
output through its original Assignment, admission, completed Invocation and
settled Receipt. The source must be a confirmed, bounded Planner/Plan execution
with Coordination funding and closed workspace access. Its contribution must
cover the original criteria. The kernel compares the whole supplied source with
the journal-derived source before accepting a proposal.

Completion has already revoked execution authority. Consuming the output uses
owner SessionControl and historical execution evidence; it never reissues the
grant. The contract and policy must still match, the current revision must agree,
and an invocation can be consumed only once. Execution events necessarily change
the journal digest, so meaningful prompt scope is checked separately from the
current commit boundary. A rejected proposal leaves its execution costs recorded.

CriteriaExtraction decodes atomic criteria from that paid output and derives
their origin from the exact original Assignment version. Existing criteria are
preserved. VoiClarification retains the question, probability of misinterpretation,
rework cost, proposed assumption and rationale; it compares their probability-cost
product with the selected interruption cost. NoQuestions records assumptions
instead. Ask remains a recorded P6 proposal; interactive ask/resume belongs to W3.
User refinement can retain unchanged Derived criteria and record an explicit
answer, but cannot invent or alter Derived provenance.

Extraction changes the contract. Building a plan therefore uses another admitted
Planner invocation with the new contract and targets. AsNeededDecomposition takes
one work definition from the paid output. ExplicitPlan is an experimental control
that uses a predeclared definition in its parameters, including its write scope;
the invocation remains charged even though its proposed scope is not selected.
Both enter the same kernel validator. Native inference has not been exercised:
the integration fixture uses the actual host with Scripted execution and receipts.

## Initial method, team and work

FixedMethod supports Solo and SoloWithVerifier. The latter requires at least two
distinct eligible Agent IDs; a parallel limit of one still permits sequential
production and verification. Initial Team/Membership records current eligibility,
exact roster/team-size pins, member limits, join time and reason. Subsequent
admission enforces membership. Changing to SoloWithVerifier cannot silently rely
on an existing one-member team; team revision remains W3.

The validator rejects missing criterion coverage, invalid paths/capabilities,
cycles and unknown or unaccepted dependency references. This slice commits only
one initial WorkItem without dependencies. Multi-item decomposition and plan
revision return explicit limitations rather than claimed progress.

Owner-selected IntakePolicy, Planner and ContributionPolicy changes are recorded
at a work boundary with no unresolved assignments or resource ownership. Method
changes use the same boundary. Earlier selections and decisions remain retained.
This is an application/library selection surface; the complete CLI/session loop
and recovered owner controls remain with their owning tasks.

## Choosing useful work

The kernel derives candidates from the current ledger and work definitions:
Produce without a candidate, Verify or DesignChecks for insufficient evidence,
Diagnose for contradiction, and Clarify/Research for recorded unanswered questions.
Only current checks of the current criterion version select Verify. Criteria or
task changes make the plan stale; adding a check alone does not invalidate it.

OrdinalValue prioritizes required criteria and their ledger status before kind.
FixedWorkflow prioritizes kind, including clarification before production or
verification. Each selects a feasible subset bounded by free participants, slots
and the existing purpose-budget rules. Forecast source, expected/p90 cost and
expected criterion benefit are retained as uncalibrated experiment parameters.

The consumer revalidates the candidate input and chosen subset, then records the
decision with the actual ContributionProposed events in one append. An unresolved
revoked assignment still occupies capacity. Verify excludes the producer and
requires the applicable execution capabilities. Current enforceable admission
remains Scripted/file-only; unavailable native, process or browser capabilities
yield limitations. A selected Diagnose is not a completed diagnosis.

Path prefiltering observes this session's holds. Gatekeeper/WorkspaceGuard retain
the authoritative cross-session physical checks at admission. Selection neither
reserves money nor promises that a later admission will succeed. The existing
Treasury funding rules still apply in the eventual admission order.
[W1-0012](progress-implementation.md) permits changed checks when the original
Task and criterion references are unchanged, while retaining the original plan
and contribution contracts.

## History and verification

New explicit PlanCommitted and MethodChosen calls emit version 2 with the stricter
coverage/work-boundary/parameter checks. Version 1 retains its original replay
rules but cannot be appended as a new decision, including through Journal directly.
New planning events publish exact Assignment references without changing
the reference set of old admissions. Plan scope is a replay-derived projection,
not a rewrite of previous event bytes. Historical Verify without Team and without
a result subject remains readable; the new planning consumer always supplies
Team and an exact ResultVersion for Verify.

`crates/ymp-storage/tests/planning.rs` exercises two sessions, four paid Planner
calls and one producer through Registry, Gatekeeper, host, SQLite and real
retained-byte checks. Both IntakePolicy and Planner variants commit actual
decisions. Method and contribution changes preserve their history; a candidate
selects Verify and then Diagnose after applicable contradictory evidence and a
ledger update. Forged/missing/stale sources, unsupported plan capabilities,
missing criterion coverage and cyclic dependencies are refused. User criteria
and unchanged Derived provenance survive explicit refinement.

Small policy-only variations check one available member, zero slots and exhausted
Production capacity without additional executions. The existing
`treasury::reserve_boundaries_keep_verification_and_reporting_protected` scenario
already rejects Plan funding from the verification reserve. Removing exact paid
source validation made the forged-source assertion fail (exit 101); restoring it
passed the complete new scenario in 98.84 seconds. Targeted Clippy, existing intake
and foundation consumers, and frozen replay checks passed. Independent code
review returned R1(9/10) ACCEPT. A separate regression preserves frozen v1 replay
while refusing a new v1 append; it failed before the journal version guard and
passed after restoration of the strict write boundary. Final `make verify` exited
0: legacy scan, offline workspace build, formatting, Clippy with denied warnings
and all workspace tests. The new planning scenario also passed in that run
(95.19 seconds). The task register and staged diff checks passed.

The first broad run stopped in existing corruption tests because the filesystem
was full. Removing only this repository's regenerable incremental build cache
restored capacity. An intermediate test rerun was stopped when the final append
version guard required a code change; the final checks cover that corrected code.

W1-0012 adds bounded researcher diagnosis through its own recovery consumer.
Ordinary candidate-scoped Verify/Diagnose execution still needs the W1-0014
dispatcher integration. Native models, interactive P6, full board-triggered
questions, decomposition, automatic session scheduling and post-restart
owner recovery are not delivered by this task. No empirical self-organization
benefit is inferred from Scripted fixtures.
