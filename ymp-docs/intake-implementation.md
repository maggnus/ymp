# Explicit task intake

W1-0002 connects explicit user tasks to Application and the journal. Task status
and completion evidence remain in `tasks/records/W1-0002.json`.

## Data and entry points

`ymp-domain/src/task.rs` represents Task, Goal, Assumption, Clarification,
Constraints, Pins, Criterion and AcceptanceContract. Criteria retain their kind,
weight, required flag, evidence classes and origin. Real values reject NaN and
infinity; constraints also require nonnegative budget/reserve, reserve within
budget, positive work limits and consistent team-size/roster pins. Native model
and effort strings are preserved, with actual offering resolution owned by
W1-0003. Reporting capacity belongs to the later ResourcePolicy/Budget and is not
an additional mandatory intake field.

`Application::open` accepts an explicit Task, criteria and policy selections.
It atomically appends SessionOpened and CriteriaCommitted. Invalid input leaves
no session or partial intake. The original request is stored verbatim.
Application returns an in-process SessionControl, which authorizes refinements
through the same application instance. Opening does not invoke a provider.

`Application::refine` requires that control, the expected revision, new criteria
and constraints, a reason, and a clarification or assumption. It retains task and
contract identity and the original request. The kernel appends the note and a
CriteriaCommitted record naming the previous contract version in one batch.
A note can describe a newly introduced criterion because the complete batch
validates against the resulting criterion set. A stale, malformed or foreign-owner
request changes no events or projected state.

## Versions and replay

AcceptanceContract.version hashes its task, full criterion definitions and check
references under an explicit v1 content format. This includes goal annotations
and constraints. Criteria have their own content references, so changing a
criterion cannot silently inherit a prior criterion version. Earlier versions
remain available in the journal and reference index.

The JSON dependency enables exact float round trips: a recorded budget or weight
must retain identical f64 bits and a stable digest after decoding. The tests
include the previously failing value `98.31267718040647` in a full intake replay,
and a deterministic sample across finite f64 representations.

Replay validates the complete task/contract relationship at a batch boundary.
An annotation without the corresponding updated contract is rejected. A requested
historical prefix inside that two-event refinement is incomplete and is also
rejected; the revisions before and after the batch remain replayable. A method
decision cannot be inserted into the incomplete intermediate state.

## Scope and authority

User intake can create only User-origin criteria and preserve unchanged existing
Derived values during refinement. The [W1-0011 planning consumer](planning-implementation.md)
validates and commits new Derived criteria from accounted Planner output, using
the exact original Assignment reference. No assignment or Planner authority is
fabricated by user intake. Check registration and
validation against executable checks belong to W1-0009.

All SessionStatus alternatives are represented. This implementation only exposes
Intake for an initialized task. Running, finalization, delivery, blocking,
cancellation and recovered controls require W1-0014. The in-memory store is not
durable; W1-0016 supplies persistence. Neither acceptance nor successful execution
is inferred from opening or refining a task.

## Evidence

`ymp-runtime/tests/intake.rs` exercises the actual Application over MemoryJournal:
all criterion kinds and exact user data, contract/criterion version changes,
clarification and assumption events, old-reference resolution, invalid limits and
pins, missing/duplicate criteria, stale updates, foreign controls and rejection of
forged Derived origins. Its malformed-history case proves that a method cannot
interleave a note and its updated contract. That test failed before the guard was
added. The finite-real test in `ymp-domain/tests/validation.rs` also failed before
exact float decoding was enabled. The W1-0001 frozen fixtures still replay after
the intake projection fields and event families were introduced.
