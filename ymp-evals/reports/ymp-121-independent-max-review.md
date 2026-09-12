# ymp 0.4.0 independent final review

**ACCEPT: installed local release ymp 0.4.0, 9/10.** No blocking finding remains. This final verdict accepts durable source `baf85540e3130227c1a4942fd17301bd039b7996` and the installed executable SHA-256 `cc8ed8a755b9778a2cfcad6460880730167e0dd39ce075543ddad25cb4278f14`. The exact accepted staged bytes were published at **2026-09-12 23:40:12 UTC** and independently verified at the installed command.

The review examines immutable versioned source `c3758aa6fc6fba620c9b703a9dab1acd623066a1` in `/tmp/ymp040-max-review`, with max reasoning as required by YMP-121. The candidate merges accepted application integration, the version-only change and the final Opus UI source. Every compiled application/driver/bridge file outside TUI is identical to the independently accepted YMP-126 candidate `a04aef41e003609c42c81ac5b698c0ca2a208274`; workspace and lockfile versions change from 0.3.0 to 0.4.0 without dependency upgrades. TUI source is byte-identical to `b6eb516`, independently accepted by Claude Opus 5 max for 0.4.0 at 9/10. Accepted integration `495502f` and documentation/status commit `baf85540e3130227c1a4942fd17301bd039b7996` preserve all 158 candidate source hashes on durable main.

The approved intent digest remains `4479e919c5b47934e69e81ac493d16e90a0dc420a9a370382e79bca3f3c754d9`. No owner request, intent, application home, main source or tracker was edited by this reviewer. Source inspection, independent evidence audits and one focused offline confirmation-policy replay took place in isolated review storage. Development/reviewer model use is separate from the application workloads, which made no native model requests.

## Requirement coverage

[requirement-matrix.json](release-040-review-evidence/requirement-matrix.json) enumerates **164 acceptance clauses across all 26 tasks in the canonical release plan**, retaining each exact criterion and its implementation/evidence references. This includes 25 implementation/verification tasks and the YMP-116 runtime-contract design. Its six documentary criteria are checked separately and are not counted as implemented functionality. Research and planning are also distinct from delivered behavior. Seven paused tasks remain outside this release: YMP-108, YMP-124, YMP-201, YMP-202, YMP-203, YMP-204 and YMP-301.

| YMP-121 clause | Reviewed evidence and current conclusion |
| --- | --- |
| 1. Complete integrated universal suite | The release-built driver passes all four workflows and thirteen protocol cases on exact `c3758aa`; no skipped adapter. Independent final UI acceptance completes the feature dependency conclusions without changing the tested source. |
| 2. Release binary, bridge, help/demo and clean install | Locked release builds and Claude bridge build/check/test pass. Temporary installer, help, version, direct-directory demo, regular-file backup and repeat-install checks pass. The durable-main executable is now installed with identical accepted bytes, verified command target/default bridge and successful installed version/help/demo checks. |
| 3. Bounded native smoke procedure | The release-verification guide records exact observed installation versions, explicit low effort for the two proposed providers, separate authentication, one sequential call each, deadlines, a reported-token stop threshold, cancellation and the incomplete-accounting limitation. It requires revalidating metadata before any quota request. |
| 4. Native calls require explicit quota | No application native inference was authorized or run. This satisfies the authorization constraint; it does not establish successful native task execution. |
| 5. Truthful compatibility and scope | Release notes distinguish scripted runtime checks, installed-provider metadata and unrun native inference. No measured quality improvement, price advantage, strict opaque-native token ceiling or production isolation is claimed. Publication evidence binds the installed artifact separately from prior preparation results. |
| 6. Migration, recovery and locations | Existing schema migrations retain historical unknown/partial records, reject newer schemas and preserve captured authority/policy. Missing unknown-usage policy defaults to Stop. Actual restart and location scenarios validate unchanged accepted files, revoked grants and original durable locations. Docs state the MVP direct-directory/no-rollback limits. Installation verification is now complete, supporting the final announcement. |
| 7. Independent max review | This review provides the exact-source requirement matrix, source/data audits and final evidence decision. UI implementation/review is reserved to Claude Opus 5 max as required. |
| 8. Replaceable subsystem consumers | All six promised categories, the three distinct knowledge policies and the workspace-policy seam have typed interfaces, actual public consumers and positive/adverse evidence. Final-source tests exercise them; a nondelegating checker was independently replayed on this exact version. |
| 9. Public stdio interoperability/lifecycle | Actual official Python MCP SDK 1.28.1 walks the release executable through 245 calls, scoped/configured-contract execution, read-only observations, cancellation/resume, stale/foreign references, SIGKILL recovery and blocked/broken-output shutdown. Largest reply is 73,149 bytes. |
| 10. No expensive native test defaults | Unattended native-wire cases run explicit Python/native protocol fixtures; Claude SDK transport replaces its executable with the checked-in fixture. Driver and public MCP workloads are mock/scripted. Max/xhigh values in fixture data do not launch models. |

The underlying delivery clauses cover the documented runtime transition/authority contract (116), durable provenance (101), budgets (102), native retry handling (103), factual narration fallback (104), metadata-only filename enumeration (105), scoped task-content memory (106), recovery disclosure (107), agent identities and native settings (109/111/127), bounded allocation (110), board transitions (112), incremental knowledge/correction (113/114), useful concurrency (115), independent acceptance and confirmation (117), visibility (118), fixture discrimination (119), assignment authority (120), replacement interfaces (122), public MCP (123), executable contracts (125), the trusted driver (126) and long metadata paths (128). The contract's normal, qualitative/unconfirmed and interrupted-work walkthroughs agree with the actual universal scenarios. UI conclusions come from the separately required Opus review, not from backend test results.

## Source and actual evidence audit

All **158 files** in the parent's final source manifest match both the independent worktree and Git `c3758aa`. All completed check-log hashes and all three published candidate artifact hashes match the actual files. The universal driver additionally binds all Rust/Cargo/workload source and all fixture/checker bytes to its build. Its source-tree SHA-256 is `9e3ede2c66c71d2a51fa6fb8cc9d47fd124bb52a6366cf617b7f10c65ad0bb39`.

The exact release-source full run took place at **2026-09-12 23:11:30–23:11:44 UTC**, with `run.json.complete: true`. The 232 retained files include selected non-Git task directories, actual outputs, separate metadata homes, native/runtime journals, nullable metrics, ID/source mappings and validator results. Independent SQLite integrity and parity inspection covers **18 databases, 682 task/assignment/invocation/decision records and all 79 protocol source mappings**. Every exported complete history matches its durable event sequence. Authority cleanup and the later location session are explicitly included. See [final-driver-record-audit.json](release-040-review-evidence/final-driver-record-audit.json) and [final-driver-file-hashes.json](release-040-review-evidence/final-driver-file-hashes.json).

No expected-answer leakage or fabricated completion receipt was found. The prior independent YMP-126 review also changed the actual workload writer to produce an incorrect document: two approving scripted reviews still yielded two failed trusted checks, two task rejections and zero acceptances. Compiled-source/checker drift, extra and reordered transitions, duplicate credit/closure/revocation and missing provenance were separately rejected. Those driver/validator files are unchanged in the release candidate.

The exact-source checks are:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --workspace` | 419 passed, zero failed; two default ignores described below |
| Claude bridge install, type check, tests and build | Pass; 14 tests |
| Explicit native catalog fixtures including the SDK fixture | 10 passed, zero ignored |
| Locked release build | Pass |
| Actual release driver | 17/17 passed |
| Official MCP SDK walk | Pass; 245 calls, actual protocol 2025-06-18 |
| Reviewer-owned different-checker replay | Four modes passed through one actual Engine test |

The default ignores are the Claude SDK native-catalog fixture, subsequently run successfully in the explicit ten-case suite, and the manual fixture-retention helper used for the interface terminal walk. Neither is a skipped universal scenario. The reviewer inspected the source-bound full suite logs rather than duplicate those full suites.

The actual SDK environment was independently checked with `importlib.metadata`: `/private/tmp/ymp-mcp123-sdk/bin/python`, Python 3.14.3, `mcp` 1.28.1; interpreter SHA-256 `cbf84109626aa1013bbe408fbb9590bd0f1c1548f038b2221c6b8b87de26ca43`. This confirms the installed client, not only the test script's printed label. Read tools add no invocation; contract mutation through tool arguments is denied; changed launch contracts cannot replace captured resume authority. Signal, EOF, undrained stdout and broken-pipe controls retain terminal accounting and revoke grants. The count of polling calls varies with executable speed and is not a fixed acceptance threshold.

## Replacement behavior and preserved runtime controls

The tested consumers are the actual `Engine` entry points and owning subsystems: `ExecutionBackend`, allocation/resource policies, `BoardProposalPolicy`, knowledge retrieval/proposal/correction policies, `ConfirmationChecker` and `WorkspaceAccessPolicy`. Policies do not receive a mutable Store shortcut. Runtime validation checks captured pins, capabilities, current task/plan/result/source versions, reviewer independence, scope and reservations before committing authority or supported knowledge.

Positive replacements change actual behavior: selected native effort, resource controls, claim ordering and executor, retrieved candidates, unconfirmed lesson proposals, correction selection and serialized access. Adverse replacements cannot exceed runtime ceilings, activate misbound knowledge or narrow a backend's enforceable access. The workspace test compares identical read workloads and backend under default versus stricter policy, observing concurrency two versus one through held barriers and durable conflict decisions.

The independent checker replay copies the previously accepted `DifferentChecker` source byte-for-byte and changes only path dependencies and five workspace lockfile versions to use this release. It never delegates to the builtin checker. Raw success produces one qualified observation; missing exit status and mutation of an input or artifact produce zero and no confirmed acceptance. [checker-probe-binding.json](release-040-review-evidence/checker-probe-binding.json) and [checker-substitution.log](release-040-review-evidence/checker-substitution.log) retain the exact binding and result. This establishes the current integrated consumer rather than relying solely on an old external probe.

## Packaging and final publication boundary

The candidate package hashes are:

| Candidate artifact | SHA-256 |
| --- | --- |
| Release `ymp` | `3595d2b5dce3d59604533c8c03c660b8355d4a0a8ff75fe89ac5f41c468f2b2e` |
| Release `ymp-eval-driver` | `e04a42e70ec449696561391dce2357dbe4a6bca3e2c85caa7934cfcdc70c74dd` |
| Claude `dist/index.js` | `002c8b154f8b5b9b8b2f2bf67ca02cff25dc8dcee3d994ae798f3b46753f3165` |

Temporary installation actually links to the tested candidate, reports `ymp 0.4.0`, preserves the preceding regular executable byte-for-byte, and leaves that backup intact on repeat installation. The mock demo writes exact `Hello from ymp\n` bytes to the selected directory; its metadata store passes integrity checking and contains only demo-provider invocations. No owner installation has yet been replaced by this evidence.

`Engine::new` embeds a source-derived default Claude bridge path. The parent resolved the packaging concern by rebuilding from `/Users/maggnus/Code/ymp2` into a separate staging target. All 158 source files match both Git `baf8554` and accepted `c3758aa`; the old owner command remained byte-identical while this build and review ran. The permanent bridge's entry, settings, resources and usage modules, package metadata and lockfile match the previously tested candidate. No temporary checkout bridge will be published.

The accepted durable package is:

| Component | Exact location and SHA-256 |
| --- | --- |
| Staged executable | `/Users/maggnus/Code/ymp2/target/ymp040-staging/release/ymp` — `cc8ed8a755b9778a2cfcad6460880730167e0dd39ce075543ddad25cb4278f14` |
| Staged driver | `/Users/maggnus/Code/ymp2/target/ymp040-staging/release/ymp-eval-driver` — `37372dad0628e5e561a8b8c32e139b9df858203c8eb25cb84f611c27b16d0204` |
| Default Claude bridge | `/Users/maggnus/Code/ymp2/ymp-bridges/claude/dist/index.js` — `002c8b154f8b5b9b8b2f2bf67ca02cff25dc8dcee3d994ae798f3b46753f3165` |
| Installed command | `/Users/maggnus/.local/bin/ymp`, pointing to `/Users/maggnus/Code/ymp2/target/release/ymp` containing the exact accepted executable |

All ten durable-build steps pass: formatting, Clippy, 419 workspace tests, ten native fixtures, locked release build, all 17 actual driver cases, official SDK walk, version, help and mock demo. Log and artifact hashes were independently checked. The staged executable independently reports `ymp 0.4.0`. Its full driver run binds Git `baf8554` and the unchanged source-tree digest; all 18 databases, 682 records and complete journals pass a fresh independent parity check. The official client again completes 245 calls with a largest reply of 73,149 bytes. The demo artifact has exact expected bytes in the selected directory. See [durable-main-verification.json](release-040-review-evidence/durable-main-verification.json) and [durable-prepublication-audit.json](release-040-review-evidence/durable-prepublication-audit.json).

This concrete acceptance preceded publication of the same staged executable with the recorded backup policy. The installed command and post-publication evidence were then independently verified as described below.

The procedure's old current-version sentence is corrected on `baf8554`. The parent records publication and final tracker status in evidence/documentation-only updates while preserving the accepted application source hashes. No additional native inference is needed for the documented offline/metadata release claim.

The final Opus follow-up independently ran 139 TUI tests, 18 discriminating mutations and actual read-only terminal walks at representative sizes. Both earlier list/record outcome and declared-source findings are closed, along with captured reservation/unknown-usage visibility. All 15 follow-up evidence file hashes and the accepted UI tree were independently verified here; see [ui-acceptance-binding.json](release-040-review-evidence/ui-acceptance-binding.json).

That review retains four nonblocking observations: an older observed-token phrase includes zero overshoot wording; a true strict-bound branch is unreachable because 0.4.0 always records false; normal Engine assignments inherit the default reservation while explicit reservations are available through trusted admission; and three knowledge-availability wording variants were not terminal-walk covered. These are not omitted required capabilities or native guarantee claims. The earlier YMP-126 diagnostic note also remains limited to a failed-case JSON snapshot; complete SQLite evidence and all successful required bundles are intact.

## Installed release confirmation

`/Users/maggnus/.local/bin/ymp` is a symlink to `/Users/maggnus/Code/ymp2/target/release/ymp`. Independent hashing confirms the exact accepted `cc8ed8a7…` bytes through both the command and target; running the installed command independently returns `ymp 0.4.0`. The published driver retains `37372dad…`, and the durable default Claude bridge retains `002c8b154…`. All 158 accepted source file hashes and the approved intent digest remain unchanged after publication.

The previous executable is preserved in `/Users/maggnus/.ymp2/backups/release-0.4.0-20260912T234012Z/ymp.previous`. Its independently checked SHA-256 is `9490cc05a74e0f7544316de5ceeeeeabc53e5ca4cc098f9a9208c16567af6849`, equal to the pre-installation executable. Installed version/help/demo checks all exit zero. The fresh installed demo writes exact `Hello from ymp\n` bytes only to `/tmp/ymp040-installed-mzhwfbyn/work/greeting.txt`; its separate metadata database passes integrity checking and contains eight demo-provider invocations. It makes no real model request.

[installed-release-audit.json](release-040-review-evidence/installed-release-audit.json) retains the independent command/byte/path/backup result and post-install log hashes; [publication.json](release-040-review-evidence/publication.json) records the publication. All 164 release-plan clauses are accounted for, with the documentary design distinguished from implementation. No implementation, installation or independent review blocker remains. Seven paused research/post-MVP items remain paused and do not become completed work through this verdict.
