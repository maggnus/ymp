# YMP-126 runtime correction independent R2

13/09 05:34 HKT, 2026-09-13 (2026-09-12T21:34:42Z) — **ACCEPT R2(9/10)** for runtime correction `1bdea888169e680e69ecf47792abcc5f53838400`. R1 F1 and F2 are closed; no bounded runtime blocker remains. This is not acceptance of YMP-126's seventeen-case driver or final release composition.

Review used immutable snapshot `51b44fc8da828e27a8562467e5316e3e35b37ae4` at `/tmp/ymp126-runtime-r2-review-kisib_im/worktree`. Its follow-up commit changes only pending driver/adapters; the runtime/core/storage/provider/workspace sources are identical to the reviewed runtime commit. The original R1 report and two failure transcripts remain unchanged at `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp126-runtime-review-codaf9v5/`.

## F1 — Actual context cannot be understated

`Engine::try_reserve_workspace` now measures the actual request prompt plus profile instructions against the captured purpose-specific character allowance before acquiring access. `WorkspaceReservation::admit_reserved` additionally requires exactly one Prompt and one ProfileInstructions reference, each with the actual digest and exact character count. Missing, zero, understated or duplicate evidence cannot substitute for the supplied request. Those checks precede TeamServer admission and grant/spend creation.

The seven public workspace tests independently pass. They reject the original 128001-character over-limit shape, combined prompt/instruction overflow, absent/zero/understated prompt counts and missing instruction evidence; an ordinary exact-bound request still admits and executes once. Changed requests, replay, foreign identity and expired capability remain denied.

An additional independent external consumer checks both captured phase boundaries: plan 32000 and conversation 128000 characters. Exact prompt-plus-instruction totals acquire; one extra actual character rejects with `context_limit`, unchanged complete trace and zero backend calls. This prevents the original R1 attack from reaching the stage where its false zero count could matter, while preserving ordinary boundary values.

## F2 — Owned project exclusion lasts through reservation lifetime

`WorkspaceOwner` is publicly nameable but not publicly constructible; its fields and constructor are private to the runtime. It owns the existing Store project lock and is tied to the canonical application home, project ID and captured directory. Public acquisition returns an Arc, and every reservation retains that Arc. A mismatched owner fails before reservation or authority changes. Normal Engine run/follow-up retain the same owned guard; the distinct-task follow-up path drops its old context/session/project guards before starting another run.

Reservation Drop ends any remaining TeamServer capability first. Field destruction then releases its process-local access lease before dropping the owner Arc, so the project OS lock outlives resource/authority cleanup. There is no ambient ownership cache or replacement lock implementation.

The actual child-process exclusion test passes. A separate independent lifetime control is stronger: it acquires a writer reservation, drops the caller's owner Arc, and proves both a fresh local acquisition and a real child process still cannot obtain the project owner. After the reservation drops, a new child acquires the owner and a writer reservation for the same path; another parent acquisition also succeeds. No native invocation is needed or fabricated for this ownership proof. A foreign-home/project owner is separately rejected with unchanged trace.

These checks establish the same application-home/project exclusion contract as normal Engine operation. They do not claim a global lock namespace across unrelated application homes or an OS sandbox against arbitrary user-owned code.

## Preserved runtime and ledger behavior

The three original variable-token storage tests independently pass again. The token accounting source is unchanged by this R2: competing database connections admit only affordable reservations; zero/oversized values cannot exceed captured limits; live review spend/protection is counted once; completed review releases unused protection; partial closed usage remains a stop; overflow is denied. Historical spend is not refunded.

The existing public controls preserve no-queue/no-spend deferral, actual Engine waiting on the same coordinator, request/backend binding, one invocation per reservation and grant revocation before access becomes available. A normal Engine artifact/memory/session consumer independently passes. An added public negative control rejects a high effort overriding the captured low pin, an excessive timeout and both native-turn/output ceilings, preserving the complete trace and zero backend calls.

## Actual commands and evidence

All commands below exited **0** on the immutable snapshot or its external path-dependent consumer. Build controls were `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_BUILD_JOBS=4`.

| Command / consumer | Result |
| --- | --- |
| `cargo test --offline -p ymp-runtime --test workspace_reservation -- --nocapture` | Seven registered tests, including the real child-process helper path, pass |
| `cargo test --offline -p ymp-storage --lib variable_ -- --nocapture` | Two variable-ledger tests pass |
| `cargo test --offline -p ymp-storage --lib live_review_reservation -- --nocapture` | One review-ledger test passes |
| `cargo test --offline -p ymp-runtime team_produces_verified_artifact_memory_and_durable_session -- --nocapture` | One normal Engine consumer passes |
| External `--test controls review_ -- --nocapture` | Phase boundaries, foreign ownership and retained-owner/cross-process release pass; helper is exercised by two real child runs |
| External `--test controls review_owned_admission_keeps_pins_timeouts_and_native_control_ceilings -- --nocapture` | Captured pin and three resource override denials pass |

External manifest: `/tmp/ymp126-runtime-r2-review-kisib_im/probes/Cargo.toml`; target directory: `/tmp/ymp126-runtime-r2-review-kisib_im/worktree/target`. Its test source copies the committed public fixture and adds review assertions; production and fixture source in the snapshot were never modified. R1's anonymous external test sources were not available as retained files, so this review does not claim byte-for-byte replay of those old sources. It exercises the same reported oversized-request and two-process attacks through the new owner-required API and adds explicit boundary/release controls.

Logs are retained beside the snapshot as `workspace-public.log`, `variable-ledger.log`, `review-ledger.log`, `normal-engine.log`, `independent-controls.log` and `independent-pins.log`. `checks.json` records the original command exits; `review-evidence.json` records source/test/log hashes. Every recorded source file matches the immutable Git snapshot. The author's targeted pass and production Clippy logs were also inspected; no duplicate full workspace suite or seventeen-case driver run was performed.

Only this external review report/evidence and disposable independent test crate were written. Source, main, sibling writers, UI, intent, task registry, real application homes and credentials were untouched; no native inference or commit occurred. Runtime R2 can proceed to integration, while driver adapters/exporter validation, final combined checks and release acceptance remain outstanding.
