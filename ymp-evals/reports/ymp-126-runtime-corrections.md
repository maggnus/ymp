# YMP-126 production corrections for independent review

This checkpoint is integration preparation, not completion of YMP-126. The immutable universal fixture exposed a token-reservation limitation in YMP-102, and the one-shot concurrency scenario needed an owned public admission seam shared with the actual Engine. The parent authorized both bounded corrections. No fixture expectation was changed.

## Token admission

`AssignmentRecord.token_reservation` is an optional positive request no greater than the captured `ResourceLimits.invocation_tokens` ceiling/default. `ResourceLimits.review_reserve_tokens` optionally captures a combined review allowance; absent values retain the previous calculation. `SessionBudget.protected_review_tokens` exposes the actual remaining protection.

Live accounting sums each assignment's requested allowance less its observed use. Required review's observed spend and remaining live reservations reduce its protected balance once; review reservations also remain included in the shared live total. Completing the required review count releases unused protection when no task/final-review obligations remain. Historical spend is never refunded. Admission checks arithmetic overflow, captured ceilings, concurrent reservations and the existing unknown/partial-usage stop. These are accounting allowances, not hard native token limits.

The unchanged budget fixture passes at `/tmp/ymp126-budget-corrected-second-20260912`: observed charges10,5,50,15 total80; agent a65/b15; three denied requests start no invocation; live reservations end at0 and20 units remain. Its resource-only work is an unprivileged read-only `execution` turn, rather than a producing task claim. Normal artifact workflows continue through Engine task execution and acceptance.

Three storage controls test competing independent connections, zero/oversized requests, live review counted once, release of unused review protection, preserved partial usage and checked-overflow denial. Logs: `/tmp/ymp126-budget-controls.log`. The earlier actual failure remains at `/tmp/ymp126-budget-gap-20260912` and in checkpoint4634766 history.

## Owned workspace admission and execution

`Engine::try_reserve_workspace` returns either an owned boxed `WorkspaceReservation` or an immediate `WorkspaceWait`. Backend access is derived from the installed execution implementation and runtime policy, with captured directory, native profile/provider, settings, controls, eligibility and capacity validation. Deferred probes create no assignment, grant, spend or queue.

The ordinary Engine uses the same derivation/coordinator helpers. Reservation admission binds assignment and prompt to the actual request and the existing TeamServer budget/grant transaction. `WorkspaceReservation::run_turn` executes that bound request once, permits continuation context without inheriting authority, validates the issued team capability and rechecks actual backend access. Changed requests, widened access, expired authority or replay fail before native work. Drop revokes any remaining bound capability before releasing resource ownership. No separate lock simulator or evaluation-only engine exists.

Four public tests cover immediate writer/reader conflicts, immutable capacity, no deferred side effects/queue, rejection of false scope and forged provider configuration, actual Engine waiting on the same reservation, identity binding, changed-request/replay rejection and capability expiry before release. Log: `/tmp/ymp126-reservation-bound-run-tests.log`.

## Remaining work

Ten universal cases have passed during preparation. Remaining adapters, exporter preflight corrections, full combined checks and final independent review are still required. In particular, actual per-case backend/build hashes and unexpected protocol-event preservation are being strengthened following the independent read-only preflight. No model-quality or cost-savings claim follows from these scripted runs.

## Independent runtime R1 corrections

The independent read of812b101 confirmed two public-seam defects: caller counts could understate an over-limit actual prompt, and the public reservation did not require the normal process-wide project lock. Both defects are corrected in the following checkpoint; the driver is still incomplete.

The signature now requires an opaque `Arc<WorkspaceOwner>` created by `Engine::acquire_workspace_owner(session)`. It owns the existing `Store::lock_project` guard, validates the exact application home/project/directory, and is retained by each reservation and normal Engine run context. There is no ambient ownership cache or alternative locking implementation. Reservations inside the owned run share the existing coordinator; another process cannot obtain the guard for the same app home/project. Normal run/follow-up keep their existing project exclusivity.

Actual prompt plus profile-instruction characters are checked against the captured purpose limit before reservation. Admission requires exactly one matching prompt and instruction context record, each with its real digest and character length. Missing, zero or understated caller counts fail before grant/spend. Seven public tests now pass, including actual128001-character rejection, combined prompt/instruction overflow, incorrect evidence lengths, missing instructions and a real child-process project-lock exclusion. Existing request-binding, expiry, false-scope and shared-coordinator controls still pass.

R1 before-control evidence is retained in the independent review directory `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp126-runtime-review-codaf9v5`. Corrected control output is `/tmp/ymp126-runtime-r1-controls.log`; production clippy output is `/tmp/ymp126-runtime-r1-clippy.log`. These are bounded corrections awaiting independent R2, not a substitute for final full-suite and17-case validation.
