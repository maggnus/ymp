# Combined concurrency, allocation and knowledge integration

13/09 00:46 HKT, 2026-09-13. The combined public consumers and all 278 workspace tests pass. This is a preparation candidate for independent integration review; standalone YMP-115 R2 acceptance does not itself accept this composition.

The isolated development worktree starts at main `8ed01ac27e94a38cf88aa3f78e58d23bb6e387ba`, including accepted YMP-110/YMP-113 and the location correction mapped to `5834e7f` from reviewed source `1b76133`. Only the requested two commits were cherry-picked:

| Accepted source | Combined mapping |
| --- | --- |
| `32624efd0b666e442068601c0828aeb174735542` | `c4cf0cd8d57ff15613575238cf9554ff54a5e705` |
| `ecf12787043861964dc18ef47d9d397cf94c9c41` | `7e9a02661dd3116f61d28ab10604ceb0e06e8437` |

The three conflicts were module declarations in core/runtime `lib.rs` and runtime `engine.rs`; both knowledge and workspace-access modules remain. No ancestry from `535b619`/`2e0c8ae` or duplicate execution-identity correction was applied. The copied standalone review includes its accepted R2 appendix. The final integration correction changes two existing public test consumers and records evidence; it adds no production behavior.

The existing two-failed-siblings consumer now exercises the full composition. Three bounded producers reach their barriers concurrently. B/C fail before A is released; their retained responsibilities remain in membership, while the distinct eligible reviewer can still confirm A by exact bytes. Unsupported reviewer model settings and actual member/size ceilings remain enforced. The blocked run retains 14 reported input tokens, two partial calls, closed grants, balanced access leases and one supported producer observation. No final learning assignment runs.

The same consumer resolves A's supported knowledge by its actual acceptance, result/version and confirmation IDs, reopens storage, constructs another Engine and starts a distinct session. That session's actual provider prompt receives the captured `Contribution A` excerpt with the retained knowledge ID/version. After metadata-only relocation into an empty directory, complete outcomes remain identical. Public `follow_up("Where did you save it?")` returns the original directory and artifact path with unchanged tasks, assignments, invocations, decisions, membership, budget and observations. Conversation messages may be appended; no backend request or artifact is created by this follow-up.

The initial combined run exposed one obsolete assumption in the earlier membership/knowledge consumer: it tried to retire the accepted-result producer while that same agent still owned its failed second task. The new guard correctly rejected this with `active_responsibility` (exit 101). The consumer now first asserts that rejection and unchanged membership/spend, then resumes through public Engine inspection with exactly the original two production assignments. Only after the interrupted task is inspected does it perform the original fixed-size replacement, retired/reserved admission checks, supported-knowledge retrieval and location checks. No completion journal was synthesized to clear responsibility.

| Verification | Exit/result |
| --- | --- |
| `cargo test -p ymp-runtime --test concurrency --test state_integration --test allocation_admission -- --nocapture` | 0; all 24 tests |
| `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| First `cargo test --workspace` | 101; compiler ENOSPC before a product test verdict |
| `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --workspace` after local `cargo clean` | 0; all 278 tests |

The failed compiler log is preserved. Only this worktree's disposable target was cleaned. Source bytes did not change between required checks or the successful retry. Source manifest, timestamps, exact commands, log hashes, construction iterations and control mutations are recorded in [the evidence ledger](../../ymp-docs/research/evidence/ymp115-combined-integration-checks.json). Its complete source fingerprint is `2e1f0feaf44fbbc5dead292a270a362c52c79de30a9c15407b80a1e212831c46`.

Three deliberate controls used the identical extended consumer command, `cargo test -p ymp-runtime --test concurrency independent_probe_two_failed_siblings_keep_eligible_review_reachable -- --nocapture`. Restoring the original pre-R2 allocation source fails 101 because A remains in Review; disabling incremental retention fails 101 because supported knowledge is absent; returning the mutable project directory fails 101 at the original-directory assertion. Each source mutation was restored byte for byte before final verification. The three complete control logs are retained beside the evidence ledger. Earlier fixture construction errors are separately labeled and are not product-defect or mutation evidence.

Byte comparisons also preserve main's effective-version helper, complete location-return block and incremental knowledge/observation block. Compared runtime/storage knowledge and confirmation implementation files remain identical to main; allocation, runtime/storage responsibility guards and workspace-access modules match accepted standalone R2. All execution is mock/scripted. Main, intent and task registry remain untouched; the sole inherited TUI change is the existing TaskAccess fixture default. Direct-directory MVP limits remain: this verifies bounded coordination and recorded evidence, not native narrow-path isolation, rollback, external-side-effect recovery or production publication.
