# YMP-117 integration review

12/09 21:47 HKT — **I1(9/10) ACCEPT** at `cbbddcb7529e56a22a10d0e58dc487ef19907c78`.

The combined YMP-102/YMP-120 admission and grant paths, YMP-106/YMP-107 changes, and accepted YMP-117 correction preserve the reviewed confirmation contracts. No integration defect remains in this bounded review. All 220 workspace tests pass; the ten original confirmation probes and thirteen selected integration probes pass. Code, evidence, and the offline public consumer each score 9/10.

## Compared sources

The review compared the candidate with accepted YMP-117 `0af7287470a53de82224a0ac6d642198153cd627` and accepted admission/authority base `f215df3`, then inspected the adjacent merged blocks and the explicit-ordinal correction at `97e8320`.

Mechanical equality checks establish:

- `ymp-core/src/confirmation.rs`, `ymp-runtime/src/checker.rs`, `ymp-runtime/src/engine/confirmation.rs`, and `ymp-storage/src/confirmation.rs` exactly match accepted YMP-117 after replacing the Rust name `CheckOutcome` with `ConfirmationCheckOutcome`.
- The complete runtime `execute` and `verify` functions match accepted YMP-117 byte for byte.
- The result-context capture block matches accepted YMP-117 byte for byte and precedes the mutable assignment passed to joint admission.
- Serde variant names and the confirmation evidence JSON remain unchanged by the Rust type rename. The separate YMP-107 recorded-check display type retains its existing name.
- Storage still invokes confirmation validation in the decision transaction, while budget reservation, invocation identity, and grant issuance remain in the combined admission transaction. Invocation completion revokes its grant before runtime review/acceptance records are committed.

The final fixture commit `cbbddcb` changes only `ymp-tui/src/tests.rs`: recorded-check fixtures are submitted/awaiting review with no invented reviewer, rather than directly seeding accepted tasks. Its test assertions are unchanged. The parent confirmed this was the intended Claude Opus 5 max correction before final workspace validation. No production UI code changed.

## Consumer and adverse evidence

The original ten confirmation probes retain all their assertions. They pass against the integration checkout, including forged final acceptance, contradictory/reused review bindings, accepted-state injection, irrelevant checks, failed typed evidence, input freshness, narration freshness, and different injected checker behavior. Full before/after JSON traces for the three decision forgeries remain identical on rejection.

Two prior public consumer extensions also pass: an older aggregate cannot be accepted after a real resume creates a new aggregate; and all twelve confirmed/unconfirmed × normal/failed narration × unchanged/artifact drift/input drift combinations return their expected result. Unchanged results remain usable at their actual grade. Persistent drift blocks delivery and records invalidation.

A new integration probe walks six public `Engine::run` cases, combining each accepted grade with invocation ceilings 7, 8, and 9:

| Invocation ceiling | Observed behavior at both grades |
| --- | --- |
| 7 | Paused after 5 admitted calls; protected review allowance prevents production; no final acceptance or reputation credit |
| 8 | Completed after 8 admitted calls; final review succeeds and narration admission is denied; fallback preserves the correct confirmed/unconfirmed grade |
| 9 | Completed after 9 admitted calls with normal narration and the correct grade |

Every admitted assignment has one correctly bound grant, every grant is revoked by return, and no invocation remains in flight. Each candidate/final review references the exact submission ID and digest captured in its admitted assignment. Grant revocation precedes the associated review decision in the journal. Confirmed producing outcomes receive exactly one observation; unconfirmed outcomes receive none. The 7-call ceiling is the discriminating adverse case for the combined admission path; the persisted review forgery inputs remain adverse cases for the confirmation boundary.

The required workspace suite additionally reruns independent aggregate review eligibility, partial evidence coverage, idempotent source attribution, legacy-observation exclusion, cancellation/recovery, explicit historical-ordinal rejection, joint reservation/grant rollback, grant replay denial, and the four corrected recorded-check UI scenarios.

## Commands and results

Repository commands ran in `/var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-release-work-h6oq_rys/integrate_confirmation` at the final fixture commit.

| Command | Result |
| --- | --- |
| Accepted-source normalized equality checks and merged-block inspection | Exit 0; equality conditions above hold |
| `cargo test -p ymp-runtime -p ymp-storage -p ymp-core` | Exit 0 |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; 220 tests pass, including 60 runtime, 26 storage, and 73 TUI unit tests |
| External command with `--skip r2::` | Exit 0; original 10 probes pass |
| External command with `--skip r2_fresh_review --skip r2_accepted_precursor` | Exit 0; 13 selected probes pass, including twelve delivery and six combined-admission cases |
| Full JSON equality checks for rejected decision captures | Exit 0 |

The external command prefix is:

```text
cargo test --manifest-path /var/folders/cw/7pn8sb3x6bj69g7j2d8f_ss00000gn/T/ymp-117-integration-probes-7coo4s98/Cargo.toml -- --nocapture
```

The original probe source changes are limited to the confirmation type rename, dependency paths, and the parent-approved mechanical addition `Limits.resources = None`. Assertions are unchanged. The existing two R2 storage-fixture generators are intentionally not part of this integration run; the accepted storage implementation is mechanically identical, and the integrated workspace tests exercise its mutation guards. `src/integration.rs` contains the new combined consumer probe. Full traces and consumer outcomes remain in the external crate's `captures/`, including `integration-budget-*.json` and `r2-delivery-*.json`.

## Limits

This verdict covers composition of the accepted changes, not a new review scope. Backend replacement YMP-122 and later release integration are outside this candidate. No real provider inference, credential reads, bridge execution, interactive TUI walk, intent edits, task-registry edits, or production/test source edits occurred during this review. Native adapter tests in the workspace suite use unattended fixtures. Bridge code is unchanged by this integration relative to the accepted admission/authority base. The only checkout write is this report.

Ledger: `I1(9/10) ACCEPT 12/09 21:47 HKT — accepted confirmation composed with admission, grants, memory and recorded checks → unchanged original probes, mechanical comparisons, combined consumer cases and all required checks pass → integration accepted`.
