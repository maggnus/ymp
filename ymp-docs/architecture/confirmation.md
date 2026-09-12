# Acceptance and confirmation

YMP-117 implements the [runtime contract](runtime-contract.md) using additive typed JSON fields in the existing immutable decision journal. SQLite schema 3 and historical records remain readable. The `SessionTrace` export includes the contracts, result versions, reviews, observations and their raw record IDs.

An independent review records its decision and rationale against an exact result version. Its approval is an assessment, never confirmation. Runtime acceptance separately records `confirmed` or `unconfirmed`. Both accepted grades can satisfy dependencies. Runtime summaries identify the grade, including when final narration fails. An unsupported qualitative result can be accepted and used without earning reputation.

## Trusted checks

A Rust client can populate `Engine.acceptance_contracts` before `Engine.run`; executable clients can supply the same trusted contracts through configuration as described in [executable acceptance contracts](executable-acceptance-contracts.md). Contracts are captured before any agent invocation and are immutable on resume. Each contract names a task title, all relevant acceptance criteria, artifact paths, supplied input paths, and the precise criteria covered by each check. Natural-language requests do not automatically create objective contract authority.

Agent-proposed plan commands still run as acceptance checks, but neither their success nor citations and agreement in model text install a trusted contract. A passed `true` command cannot establish task quality. General tasks without a trusted objective contract remain unconfirmed.

The built-in check executor supports:

- Exact artifact bytes, including newline differences.
- Artifact bytes equal to an actual supplied input file, captured before execution.
- A client-declared validator program with explicit arguments and pinned executable/verifier-file digests. Arguments are passed directly, and `{workdir}` expands to the selected directory.

A declared command's meaning comes from the trusted client's explicit criterion binding. The runtime does not infer shell semantics. Clients must list the validator code dependencies in `verifier_files`, keep those files outside the agent's writable directory, and declare the actual dataset inputs. For the universal fixtures, a trusted client can invoke `universal.py artifact` with the scenario's check and criterion IDs. The integrated YMP-121 driver and its receipt exporter remain separate work.

The runtime captures initial input bytes, candidate artifact bytes, before/after identity, raw stdout/stderr, exit code, checker identity/version and criterion coverage. Input and artifact paths must remain inside the selected directory. A changed input or verifier, changed artifact, missing process result, failed process launch or timeout cannot produce confirmation. Explicit failing applicable evidence prevents acceptance even if every agent approves. A failed infrastructure operation produces no negative competence observation.

Only covered criteria are confirmed. A result is fully confirmed when every declared criterion has applicable passing evidence and none has failing evidence. Partial evidence remains linked and inspectable; it cannot confirm uncovered quality. A final aggregate contains every accepted component's exact version, criteria and producers. One confirmed component cannot confirm another component's quality.

## Replaceable execution, fixed authority

`ConfirmationChecker` is a narrow injected Rust interface. `Engine.confirmation_checker` defaults to `BuiltinConfirmationChecker`; alternative implementations return only `CheckExecution` (exit code and raw output). The runtime owns snapshots, criterion binding, freshness checks, grading, independent review and credit. `CheckerIdentity` is captured in the trusted contract and each check record. Resuming with a different checker identity requires restoring the captured implementation. Swapping an implementation with the same identity is a trusted-client responsibility, as with an in-process provider adapter.

Storage independently checks typed assertions against captured bytes, so a replacement executor's invented success cannot pass an exact-byte assertion. For declared validator programs, executing the declared program correctly is part of the trusted checker's implementation contract. There is no dynamic library loader, sandbox guarantee, plugin marketplace or universal correctness oracle.

## Persistence and attribution

`RecordLinks.result` holds immutable result ID/version, task attempt, original result text, criteria version, producer assignment IDs, artifact snapshots and optional component submission IDs. `acceptance_contract_captured`, `result_submitted`, `check_observed`, review, task acceptance and final acceptance decisions remain separate journal entries. Reviews and acceptances link their evidence by raw decision ID. State changes and their acceptance decisions commit together.

Each review assignment captures a `Result` context reference containing the immutable submission ID and digest. Storage binds its single assessment to that reference, its exact task scope and its candidate/final purpose. Final acceptance requires the latest recorded aggregate and its actual final review, covering every current accepted task; decision kind and outcome must agree. Candidate acceptance preserves the submitted task definition, producing assignment and exact result text. Plain writes cannot reopen an accepted attempt, and changed content requires a fresh result version and review.

Storage rejects mismatched result content/version, crossed review/evidence links, self-review and unsupported acceptance grades. A final reviewer must differ from every aggregate producer. If all available agents contributed, the session stays blocked with a `final_review_pending` decision; no self-review invocation is started. Changing an artifact during review rejects that attempt and requires a new producing attempt and independent review. Changing it after acceptance makes the current confirmation grade unconfirmed; final assembly records invalidation rather than silently using the stale version. After optional work and both successful and failed narration, `Store.result_is_current` rechecks current task bindings, artifacts and supplied inputs before delivery. Persistent changes record invalidation, preserve historical acceptance and supported observations, and return a blocked outcome requiring a fresh result and independent review. Older records remain inspectable; new transitions require complete version bindings.

`Store.observe_confirmed` credits only a confirmed, completed producing invocation using its actual execution configuration. Its observation identity is fixed by result/version/agent, and the observation and attribution decision commit together. Replays produce no extra credit. Planner or reviewer agreement earns no credit; this task does not infer their competence from another agent's checked result. Historical observations remain available through `observations()`, while reputation selection uses only rows with a validated `reputation_observed` attribution decision. Earlier unknown observations and grades are not silently converted into evidence-backed selection data.

Knowledge promotion and correction remain YMP-113/YMP-114 work. Their consumers can use the result, criterion, input, check and acceptance links added here; YMP-117 does not redefine memory activation policy.

## Limits and evidence

This release captures declared files up to 4 MiB each and limits stored command output to 4 MiB; truncated output is inconclusive. Snapshot checks detect changes before/after checking and at acceptance, but do not provide an OS filesystem sandbox or prevent a malicious external writer from changing and restoring bytes between observations. Native permissions and assignment grants remain separate boundaries.

Offline tests call public `Engine.run` with the existing mock provider and real local file/process checks. They cover no checks, unrelated commands, agreement-only arbitration, confirmed success, contradicting applicable checks, partial coverage, actual supplied bytes, pinned validators, artifact changes, independent final review, forged links, checker replacement, narration failure and duplicate credit. These are runtime protocol tests, not evidence of native model quality or completion of the universal fixture driver.
