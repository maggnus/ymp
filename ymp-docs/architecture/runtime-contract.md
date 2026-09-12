# Runtime transition and authority contract

Decision for YMP-116, based on the approved [intent](../../intent.md). This contract defines the minimum required for the next local release. Existing Rust package boundaries, SQLite, native adapters and team tools remain the implementation basis. Feature-specific numerical defaults belong to their implementation tasks.

## Authoritative records

| Record | Required facts | Owner |
| --- | --- | --- |
| Session policy | Goal/constraints, selected directory, captured limits, eligible pool and fixed settings | Runtime, from user input and recorded defaults |
| Task | Stable ID, expected result, criteria/checks, dependencies, current state and attempts | Runtime |
| Assignment | Stable ID, session/task IDs, agent ID, purpose, model/effort, permissions, limits, current state | Runtime |
| Invocation | Assignment ID, native runtime/continuation identity, requested/sent/reported settings, timestamps, usage and coverage | Runtime and observed native events |
| Review | Result version, independent reviewer, decision, rationale and supporting record IDs | Runtime records the independent assessment |
| Confirmation | Runtime-observed deterministic/external evidence, its result/criterion scope and outcome | Runtime |
| Knowledge | Candidate/active/superseded status, applicability, source outcome and confirmation basis | Runtime, from proposals and checked evidence |

Add types in ymp-core and persistence through ymp-storage. Records may use additive tables or versioned JSON metadata; existing session/task identities and historical records must survive. A typed event record links each decision to its assignment, invocation, task and evidence rather than relying on the most recent free-text message. State changes and their events commit together. Schema migrations run transactionally and reject newer unsupported schemas; legacy evidence remains unknown rather than becoming confirmed.

## Task and assignment transitions

| Trigger | Required condition | Committed result |
| --- | --- | --- |
| Propose work or revision | Active assignment may submit proposals; session and actor are bound by the runtime | Pending proposal; no authority or task acceptance is created by the message |
| Admit work | Dependencies accepted, executor available, settings allowed, resource allowance available | Task running; new assignment/invocation identity and its bounded authority |
| Produce a candidate | Current executor and assignment match; result and affected artifacts are identified | Task awaiting review; execution authority ends |
| Accept after review | Reviewer did not produce this result; applicable failing evidence cannot be overridden | Task accepted with explicit review basis and confirmation grade |
| Reject after review | Independent review or applicable evidence rejects the candidate | Retryable task or blocked task; preserve attempt history and evidence |
| Revise/reassign | Runtime validates current task/plan version and user constraints | Updated pending work and a fresh assignment; previous grants are ended |
| Finish/cancel/fail an invocation | Terminal native state or cancellation/timeout observed | Invocation closed with usage coverage; grant revoked even when result is unknown |
| Recover interrupted work | No prior process authority is trusted; inspect actual result before replay | Fresh inspection assignment; no automatic repetition of uncertain effects |

An assignment may contain bounded native work, but authority never transfers to a later assignment. Native continuation keeps useful conversation context only; it is not a permission grant. The runtime admits all work, including planning and verification. A temporary planner can propose changes but cannot directly create privileged assignments or accept a result.

Keep existing task states where they express these transitions. Acceptance and confirmation are separate data, not a large new workflow hierarchy. An accepted result can be used as a dependency unless the task's explicit criteria require confirmed evidence. Confirmation never spreads to an entire aggregate merely because one component was confirmed.

## Permission boundary

Issue a fresh team-tool capability for each admitted assignment, bound to its session, agent and allowed operations. Permit board reads/posts and relevant proposals as appropriate. Runtime-only operations include assignment commitment, permission grants, final acceptance, confirmation and reputation updates. Agent-supplied IDs cannot override the bound caller.

Ending an assignment removes its active capability. Restart does not reconstruct a live grant from a saved native session ID. A stale token, a token for another assignment or an attempt to grant privileges through board text is rejected. The actual provider permission mode/tool set must agree with the assignment. Where the native runtime cannot enforce a requested restriction, record the limitation or reject that requested guarantee; the local team API is not an OS sandbox.

Use the existing local server and a runtime-owned active-grant map; no identity service, distributed lock system or permanent agent leader is introduced. Durable grant/transition records support recovery and audit while live capability secrets stay process-owned and out of logs.

## Acceptance, confirmation and observations

An independent review can accept a qualitative result without external confirmation. Store it as accepted and unconfirmed; preserve the reasoning basis, and do not increase reputation. Rejected or unreviewed work is not accepted simply because it lacks confirmation.

Confirmation needs observed evidence relevant to the particular result and acceptance criteria. A shell exit status by itself, an unrelated passing command, a citation invented in model text or model consensus is insufficient. Capture check inputs/results and artifact/source identity so a reviewer and the runtime can establish what the evidence actually covers. Typed artifact/data assertions or declared trusted checks provide a concrete basis; unsupported general claims remain unconfirmed. Agent-provided external-evidence references must be resolved and captured by the runtime rather than trusted as strings.

Only supported, attributable outcomes update competence, once per observation ID. Infrastructure failure or a missing result does not establish poor competence. Reviewer agreement alone earns no credit. Preserve historical observations, but keep legacy/unknown confirmation separate from the evidence-backed selection inputs. The same basis and scope rules apply to promoting general knowledge and correcting an existing entry.

## Admission and working directory

Capture session limits once. Use simple counters and in-flight reservations to prevent two assignments from spending the same remaining allowance. Bound initial work and retain resources for required review. Charge planning, communication, retries, execution, review and optional consultation to the same session. Reassignment does not reset spend.

An outer invocation can contain many native requests. Enforce native loop/output/time controls where available, retain complete/partial usage distinctions and never claim a strict token or currency bound from incomplete native accounting. Settings and budget pins are validated before invocation. No model call is needed to list available agents or enforce a counter.

Use the selected working directory and the governing workspace policy. A filesystem writer owns the conflicting resource; independent read-only work may overlap. Check the same artifact version that a review refers to; a later write invalidates that confirmation for the changed result until rechecked. Task output paths and the actual directory remain available to follow-up questions. Application metadata contains records and evidence references, not hidden replacement deliverables.

## Three required walkthroughs

1. **Document with an objective check:** admit execution with bounded write authority; record the output path; end the grant; admit an independent reviewer; execute and capture the relevant assertion on the current artifact; commit acceptance with its confirmation basis. A later location question reads the stored outcome and path without rerunning production.
2. **Qualitative accepted result:** produce the candidate; an independent reviewer accepts with a rationale; no relevant external/deterministic evidence exists. Commit accepted/unconfirmed, award no reputation and show the distinction. This is a usable result rather than a fabricated confirmed success.
3. **Interrupted execution:** close the invocation with partial usage and revoke its grant; preserve completed sibling outcomes; issue a fresh inspection assignment against actual files. Reassign only the unresolved work under the remaining budget. The old token and inherited work-role authority stay invalid.

YMP-101 implements records/provenance; YMP-120 enforces grants; YMP-117 implements confirmation-qualified acceptance; YMP-102 adds admission limits; YMP-110/YMP-112 add permitted adaptation; YMP-115 adds useful concurrency. YMP-119 prepares discriminating fixtures for these walkthroughs, and YMP-121 verifies the integrated release at max reasoning. This document is the contract, not completion evidence for those implementation tasks.
