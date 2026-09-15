# Migration plan: current runtime → self-organizing team kernel

Status: draft, 2026-09-14. Companion to the proposal [self-organizing-team-domain-model.md](self-organizing-team-domain-model.md), which the owner has not yet approved. This plan does not authorize implementation; it specifies how the migration would proceed once approved. On 2026-09-14 the owner permitted dropping tests to accelerate migration where it helps; this plan applies that permission selectively (section 7).

Language of record: English, per repository policy.

## 1. Decision summary

1. **Same project, no parallel rewrite.** The migration stays inside this Cargo workspace. A new crate `ymp-kernel` receives the kernel services and ports; existing crates become backends and strategies behind those ports. This follows the owner's library-unification direction (2026-09-13): a consolidated replacement rather than a second implementation of providers, storage or knowledge.
2. **Strangler, not big-bang.** The existing `Engine` pipeline keeps running until the `Dispatcher` reaches behavioral parity on scripted scenarios, then is retired in one step. Each phase leaves the workspace building and `cargo test --workspace` green (on the tests that remain, per section 7).
3. **Hybrid journal retained initially.** The current model — append-only `events` plus mutable entity tables — stays through the early phases. The new contract is a deterministic `SessionView` projection (rule D-4 as a post-condition checked in tests). Full event-sourced replay (R-17) is optional later and is not a migration blocker.
4. **Stage mapping.** The current code corresponds to target build stages 1–4 with parts of 5. Phases below re-sequence the target's build order ([domain model, section 10](self-organizing-team-domain-model.md)) into extraction work (phases 1–3) and new-capability work (phases 4–8).

## 2. Starting point (gap summary)

Evidence: full survey of all crates, 2026-09-14. Approximate size: ymp-core 4.4k LOC, ymp-storage 9.6k, ymp-runtime 15.3k, ymp-providers 2.6k, ymp-workspace 2.4k, ymp-cli 0.9k; ~310 test functions total.

| Target component | Current state | Disposition |
|---|---|---|
| Journal + `*View` | `events` table + mutable projections; `Store::trace()` (storage/provenance.rs) | Keep storage; add event envelope + `SessionView` |
| Registry | Native catalog scans, pool with exclusion reasons, capabilities (core/config/*) | Extract into kernel service |
| Treasury | Token/invocation budgets, reservations, review reserve (core/budget.rs, storage/budget.rs) | Keep; add CostUnits/PriceBook normalization adapter |
| WorkspaceGuard (Direct) | Scoped access algebra, `AccessCoordinator`, exclusive verification (core/workspace_access.rs) | Keep as WorkspaceProvider(Direct) |
| Gatekeeper | Admission validators inside storage seams + `admit_invocation_with_grants` | Extract; formalize op surface |
| Arbiter | Board proposals → runtime commit (core/board.rs, runtime/engine/board.rs); no bids by design | Extend with solicitations/offers/awards (phase 5) |
| AcceptanceAuthority | `AcceptanceContract`, `TrustedCheck`, evidence links, anti-self-review (core/confirmation.rs, storage/confirmation.rs) | Keep validators; add grades/belief ledger (phase 4) |
| ExperienceVault | Beta reputation gated on Confirmed; knowledge with scope/lifecycle/corrections | Keep; add calibration, trials, consequences (phase 7) |
| Dispatcher | **Absent.** `Engine::execute` is a fixed pipeline: plan → waves → review → final review → learn | Rewrite (phase 3) |
| ExecutionBackend | Codex / Claude bridge / ACP / Mock with sent/reported settings, usage, failure classes | Keep unchanged; add cost receipt normalization |
| Grants | `GrantRecord` + 7 `TeamOperation` ops + MCP token server (core/authority.rs, runtime/mcp.rs) | Keep; extend op set in phase 5 |
| PolicyRef provenance | `PolicyProvenance` chains on every decision | Keep; rename/mapping only |

## 3. Phases

Each phase lists goal, work items, exit criteria, and test action. Sizes: S ≤ ~1k LOC touched, M ~1–3k, L > 3k.

### Phase 0 — Decisions and preparation (S)

- Owner decisions required before phase 4: (a) target-architecture approval; (b) `CreditPolicy` — which confirmation grades are creditable ([domain model §12](self-organizing-team-domain-model.md)); (c) confirm the selective test-drop policy in section 7.
- Fix the relative links inside the moved domain-model document once the owner chooses whether `v3/` is a self-contained version set or references current docs.
- Register approved phases in `ymp-docs/tasks/tasks.json` (via manage.py); this plan does not create tasks by itself.
- No code changes.

### Phase 1 — Event envelope and SessionView (target stage 1) (M)

- Define the common event envelope (seq, session, actor, policy, input digest, refs) over the existing `events` table; new writes fill the envelope, old rows are read as-is (projection tolerates legacy payloads).
- Introduce `SessionView` — a deterministic read-only projection assembled from current records plus events — as the only input strategies may consume (D-1, D-4). Implemented first as a thin wrapper over `Store::trace()`.
- **Exit:** for a scripted session, two independent loads produce byte-identical views; existing storage tests still pass.

### Phase 2 — Extract kernel services (target stages 2–4) (M–L)

- Create crate `ymp-kernel` with the service op surfaces from [domain model §4](self-organizing-team-domain-model.md): `Registry`, `Treasury`, `WorkspaceGuard`, `Gatekeeper`, `AcceptanceAuthority`, `ExperienceVault`. Each op either records an event or returns `Denied` (D-2).
- Move the corresponding validators out of ymp-storage seams into the kernel services; ymp-storage remains the persistence layer behind them. Error codes are preserved where CLI/TUI surface them.
- `Treasury`: keep token accounting; add a `CostModel` adapter (PriceWeighted) that converts receipts to CostUnits via a PriceBook; token budgets remain the hard limit until phase 7 calibration makes cost estimates usable.
- `ReadinessProbe` (StaticDependencyProbe): adapt from existing native catalog/adapter readiness checks; no model calls.
- **Exit:** every admission, reservation, check registration and observation in a scripted run flows through a kernel op and produces a journaled event; no direct storage mutation from runtime code outside the kernel.

### Phase 3 — Dispatcher replaces the Engine loop (target stage 5) (L)

This is the core rewrite and the main place where dropping tests pays off.

- Implement `Dispatcher` (algo A1) in `ymp-kernel`: work-boundary loop, contribution templates via `ContributionPolicy` (default `OrdinalValue` mapped from current task-wave selection), stop rule, board-event waits.
- Map current policies onto target default strategies with behavior equivalence: `FixedMethod(SoloWithVerifier)` = today's pipeline shape; `AsNeededDecomposition` = current plan/wave decomposition; `AnyNonProducer` = current reviewer selection; `CriteriaProjection + JournalDigest` = current context assembly; `EvidenceDelta`/`RuleBasedDiagnoser`/`DiagnosisFirstLadder` = `BoundedRecoveryPolicy` extended with stall detection (A9).
- Move `Engine::plan`/`verify`/`review_recovering`/final-review orchestration into Dispatcher-driven contributions; `ymp-runtime` keeps process supervision, provider glue and MCP.
- `Engine` remains selectable (config switch) until parity, then is deleted with its tests.
- **Parity gate:** the scripted-provider end-to-end scenario (mock native protocol fixtures) produces the same acceptance outcomes, provenance records and budget events as the current Engine on the same fixture.
- **Test action:** retire the Engine pipeline tests (section 7, group D1); keep one scripted end-to-end smoke per phase.

### Phase 4 — Graded acceptance (target stage 6) (M–L)

- Extend `ConfirmationStatus` {Unknown, Unconfirmed, Confirmed} to the target scale {Refuted, Unconfirmed, Discriminated, Confirmed(basis)}; per-criterion `CriteriaLedger` with the LR-table `BeliefModel` (A8); criterion weights.
- Hidden checks (R-10): visibility flag on `Check`, exclusion from producer context and from producer assignment for the same criterion; `VerificationDesigner` and `MutationStrategy` ports with defaults per domain model §5.1; `Mutant`/mutation score wired into `verify` (A6).
- Legacy data: existing sessions keep their recorded grades; the projection maps old {Unknown, Unconfirmed, Confirmed} into the new scale without rewriting history.
- Depends on Phase 0 `CreditPolicy` decision (R-11).
- **Exit:** a scripted producer-with-faulty-result session yields Refuted/Discriminated outcomes where the current system only marks Unconfirmed; mutant runs recorded with scores.

### Phase 5 — Self-organization protocols (target stage 7) (M–L)

- `Arbiter`: solicitations, offers, awards, commitments with leases (P1, P2); extend `TeamOperation` with the target operations; retire the current test asserting that no bids ever occur and activate the existing unused `bid` admission purpose only where the `BidAssignment` path requires it.
- P3 (goal-status notices → contribution response), P4 (objections with counterexamples), P5 (context handoff digests), P6 (clarify-or-assume), P7 (independence windows).
- Default strategies: ResponseThreshold, CalibratedValuePerCost (reputation-only until phase 7 calibration), DemandDriven, CheapestAdequate, CounterexampleFirst, DifferentFamilyComparableStrength.
- **Exit:** scripted contract-net scenario — a solicitation with one idle eligible agent produces one offer, one award, one leased commitment, and lease expiry triggers reopen + diagnosis.

### Phase 6 — Independent attempts (target stage 8) — **gated**

- `WorkspaceProvider(CopyOnWrite)` and algo A10 require isolated workspace copies, which are outside the MVP workspace policy; blocked on YMP-124 (post-MVP isolation). Planned but not scheduled.

### Phase 7 — Experience: calibration, trials, consequences (target stage 9) (M)

- `CalibrationScorer` (BrierIsotonic) over recorded forecasts vs outcomes; `KnowledgeTrial` arms; `ConsequenceSource` + `regrade` (A12); `CreditPolicy` enforcement moves to `ExperienceVault.observe`.
- Knowledge stack stays; add mandatory falsifier and trial-based promotion to the existing lifecycle.

### Phase 8 — Data-driven routing (target stage 10) (M)

- `CascadeRouter` (A1.1) and `VocValue` contribution scoring replace `FixedMethod`/`OrdinalValue` defaults once phase 7 produces calibration data.

## 4. Component disposition

| Component | Action |
|---|---|
| ymp-providers (Codex/Claude/ACP/Mock adapters, failure classification, supervision, redaction) | Keep unchanged; becomes the `ExecutionBackend` implementations |
| runtime/mcp.rs `TeamServer` + grants | Keep; op set extended in phase 5 |
| ymp-workspace | Keep; phase 6 adds CopyOnWrite behind `WorkspaceProvider` |
| ymp-storage | Keep as persistence behind `ymp-kernel`; validators move to kernel services |
| core/confirmation.rs + storage/confirmation.rs | Keep validators/evidence chain; grades and ledger extended in phase 4 |
| core/knowledge.rs + reputation.rs | Keep; extended in phase 7 |
| core/budget.rs + storage/budget.rs | Keep; CostUnits adapter added |
| runtime Engine (engine.rs + submodules) | Replaced by Dispatcher in phase 3; deleted after parity |
| runtime policies (allocation/board/recovery/resource) | Become default strategies behind kernel ports |
| ymp-tui / ymp-cli | Adapt: consume `SessionView` and new event kinds; no new native discovery paths |

## 5. Data and compatibility

- `~/.ymp2` migrations remain forward-only (`PRAGMA user_version`); new event kinds are additive; previously recorded rows are never rewritten ([domain model §10](self-organizing-team-domain-model.md)).
- Mutable entity tables are re-documented as projections/caches of the journal; they may be rebuilt from events for `SessionView` purposes but stay authoritative for legacy reads during migration.
- Native catalog snapshots, provider fingerprints and grant token semantics are unchanged.
- MVP workspace policy applies throughout phases 1–5 (direct directory, metadata only, no hidden copies).

## 6. Risks

| Risk | Mitigation |
|---|---|
| Engine entanglement (4.2k-line engine.rs + 12 submodules) | Strangler switch; parity gate on scripted fixtures before deletion |
| Storage validators hard-coded at the seam; extraction changes error surface | Phase 2 preserves denial codes; CLI/TUI mapping checked in the same phase |
| Behavior drift in acceptance semantics (phase 4) | Dual-run old/new grading on scripted sessions; legacy grades mapped read-only |
| Token→CostUnits normalization errors (phase 2) | CostModel is advisory until phase 7; token budgets remain the hard limit |
| TUI/CLI coupling to current event shapes | New event kinds additive; unknown-kind tolerance verified in phase 1 |
| Test drop removes safety net where it matters | Selective policy (section 7): only pipeline-coupled tests are dropped |

## 7. Test policy (owner permitted selective drop, 2026-09-14)

Dropping **all** tests would slow the migration down: the components being kept (providers, storage invariants, budget, knowledge) rely on their tests as the only safety net while their call sites are refactored. The plan drops selectively:

- **D1 — Drop (≈210 functions, ymp-runtime):** Engine pipeline tests — inline engine tests and `ymp-runtime/tests/*` that encode the fixed plan→wave→review pipeline. They are invalidated by phase 3 regardless; keeping them green would force re-patching them twice (pre- and post-parity). Replaced by one scripted end-to-end smoke per phase and by port contract tests.
- **K1 — Keep (≈100 functions):** ymp-storage invariant tests (budget denials, provenance, confirmation evidence rules), ymp-core unit tests, ymp-workspace tests. These guard unchanged behavior of kept components and are cheap to maintain.
- **K2 — Keep (≈40 functions):** ymp-providers protocol tests (Codex/Claude/ACP fixtures, failure classification, usage accounting). Highest replacement cost in the repo; never drop.
- **C1 — Add:** per-port contract tests exercising each strategy through its real consumer with two substantially different implementations (domain-model replacement rule). Mock provider and scripted strategies provide the second implementation; no new fixtures required.
- Repository gates (`cargo fmt --all --check`, `clippy -D warnings`, `cargo test --workspace`) keep running after each change, over the remaining suites.

## 8. Sequencing

```
Phase 0 ─► 1 ─► 2 ─► 3 ─► 4 ─► 5 ─► 7 ─► 8
                          └─────────────► 6 (gated by YMP-124)
```

Phases 1–3 are the structural migration (est. L overall); 4–5 deliver the target's quality and self-organization semantics; 7–8 are data-dependent refinements. Phases 4 and 5 can partially overlap once phase 3 lands.

## 9. Next step

On owner approval of the target architecture and the `CreditPolicy` decision, register phases 1–5 in `ymp-docs/tasks/tasks.json` with dependencies per section 8 and start phase 1.
