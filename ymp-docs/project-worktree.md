# Proposed crate and module layout

The layout was checked in as an empty placeholder skeleton by owner decision,
2026-09-16. Owning tasks fill its modules; placeholders are not delivered
functionality. The [journal mapping](journal-implementation.md) documents the first
concrete consumer and its implementation boundaries. Task W1-0001 owns the initial
Rust mapping and may rename or move placeholders; later tasks own the files they are listed against
and fill them. `AGENTS.md` records this as the single exception to its
no-placeholder rule; no further placeholders are added.

The layout mirrors the approved model: `ymp-domain` follows the bounded
contexts of [section 3](self-organizing-team-domain-model.md#3-domain-structures),
`ymp-kernel` follows the trusted services of
[section 4](self-organizing-team-domain-model.md#4-trusted-runtime-services-kernel)
and the port groups of [section 5](self-organizing-team-domain-model.md#5-replaceable-strategies-port),
and `ymp-runtime` holds strategy implementations and adapters. The comment on a
line names the task that creates or extends it.

## Dependency direction

```text
ymp-cli ──► ymp-tui ──► ymp-runtime ──► ymp-kernel ──► ymp-domain
  │  │                      ▲               ▲              ▲
  │  └──────────────────────┘               │              │
  └────────► ymp-storage ───────────────────┴──────────────┘
```

The domain knows nothing about the kernel, the kernel knows no concrete
adapter, the runtime knows no presentation, and the interface crate sees only
`Application` operations and `*View` projections. `ymp-cli` stays a thin
binary: argument parsing, exit codes and non-interactive subcommands such as
the W3-0009 comparison runner, which must not depend on a terminal UI. Crates
reference each other only by relative `path` dependencies.

## Tree

```text
ymp/
├── Cargo.toml                      # W1-0001: workspace, resolver 3, edition 2024, lints
├── Cargo.lock
├── rust-toolchain.toml             # pinned: 1.98.1 with clippy and rustfmt
├── Makefile                        # make help: build, verify, task-register targets
├── tools/
│   └── legacy_scan.py              # make legacy-scan: rejects code copied from legacy-* tags
└── crates/
    ├── ymp-domain/                 # pure values and validation, no I/O
    │   ├── Cargo.toml              # sha2, serde, serde_json
    │   ├── src/
    │   │   ├── lib.rs              # W1-0001: Id<T>, Digest, Ref, PolicyRef, Proposal, Denial
    │   │   ├── journal.rs          # W1-0001: Envelope, codec, policy parameters, Method value
    │   │   ├── task.rs             # W1-0002: Task, Goal, Criterion, AcceptanceContract, Pins
    │   │   ├── identity.rs         # W1-0003: Agent, Provider, ModelOffering, ExecutionProfile, Pool
    │   │   ├── resources.rs        # W1-0004: Budget, PriceBook, Reservation, Receipt, Allowance
    │   │   ├── workspace.rs        # W1-0005: Workspace, PathLock, Snapshot, Artifact
    │   │   ├── assignment.rs       # W1-0006: Contribution, Assignment, Grant, Invocation
    │   │   ├── coordination.rs     # W1-0007: Commitment, Lease; W3: Board, Notice, Offer, Objection
    │   │   ├── plan.rs             # W1-0008: Plan, WorkItem, Attempt
    │   │   ├── result.rs           # W1-0008: ResultVersion
    │   │   ├── verification.rs     # W1-0009: Check, CheckSpec, CheckRun; W1-0019: Evidence, Review
    │   │   ├── report.rs           # W1-0013: Claim, Report
    │   │   └── experience.rs       # W5-0001: Observation, Reputation, Knowledge, Retrieval
    │   └── tests/
    │       └── validation.rs
    │
    ├── ymp-kernel/                 # trusted operations, ports, replay
    │   ├── Cargo.toml              # ymp-domain, serde, serde_json
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── journal.rs          # W1-0001: trait Journal, append with expected revision
    │   │   ├── events.rs           # W1-0001: typed payloads of section 9, one variant per event
    │   │   ├── view.rs             # W1-0001: deterministic *View projections
    │   │   ├── decision.rs         # W1-0001: first decision consumer with PolicyRef and input digest
    │   │   ├── ports/              # strategy traits, grouped as in model section 5
    │   │   │   ├── mod.rs
    │   │   │   ├── checks.rs       # ReadinessProbe, IntakePolicy, VerificationDesigner, CheckRunner
    │   │   │   ├── planning.rs     # MethodRouter, Planner, ContributionPolicy, BeliefModel
    │   │   │   ├── organization.rs # VolunteerPolicy, AwardPolicy, TeamPolicy, ReviewerPolicy, ...
    │   │   │   ├── progress.rs     # ProgressMonitor, FailureDiagnoser, EscalationPolicy
    │   │   │   ├── resources.rs    # CostModel, ResourcePolicy
    │   │   │   ├── experience.rs   # CreditPolicy, ReputationModel, CalibrationScorer, ...
    │   │   │   └── execution.rs    # ContextComposer, NarrativeComposer, WorkspaceProvider, ExecutionBackend
    │   │   ├── intake.rs           # W1-0002
    │   │   ├── registry.rs         # W1-0003
    │   │   ├── treasury.rs         # W1-0004
    │   │   ├── workspace_guard.rs  # W1-0005; W4-0001: merge
    │   │   ├── gatekeeper.rs       # W1-0006
    │   │   ├── arbiter.rs          # W1-0006 minimal award; W1-0007 P2; W3 complete
    │   │   ├── execution.rs        # W1-0017: trusted invocation observation transitions
    │   │   ├── results.rs          # W1-0008
    │   │   ├── acceptance.rs       # W1-0009 register_check/run; W1-0019 evidence; W1-0010 accept
    │   │   ├── ledger.rs           # W1-0010: CriteriaLedger, A8
    │   │   ├── plans.rs            # W1-0011: Plan validation
    │   │   ├── progress.rs         # W1-0012
    │   │   ├── finalization.rs     # W1-0013
    │   │   └── experience_vault.rs # W5
    │   └── tests/
    │       ├── replay.rs           # W1-0001: replay and refusal of malformed history
    │       └── support/mod.rs
    │
    ├── ymp-runtime/                # application assembly and port adapters
    │   ├── Cargo.toml              # ymp-domain, ymp-kernel, serde_json
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── memory_journal.rs   # W1-0001
    │   │   ├── application.rs      # W1-0002 intake operations; W1-0014 start/interrupt/recover
    │   │   ├── dispatcher.rs       # W1-0014 loop A1; W3-0008 event-driven boundaries
    │   │   ├── clock.rs            # W1-0007/W1-0017: controllable clock
    │   │   ├── readiness.rs        # W1-0003: StaticDependencyProbe
    │   │   ├── execution_host.rs   # W1-0017
    │   │   ├── team_operations.rs  # W3-0001: OperationRequest transport
    │   │   ├── attempts.rs         # W4-0002
    │   │   ├── consequences.rs     # W5-0005
    │   │   ├── experiments.rs      # W3-0009: comparison runner; W6-0005 extends it
    │   │   ├── backends/
    │   │   │   ├── mod.rs
    │   │   │   ├── scripted.rs     # W1-0017
    │   │   │   ├── codex/          # W1-0018: mod.rs, protocol.rs, usage.rs
    │   │   │   ├── claude.rs       # W6-0001 (scheduled in the W3 window)
    │   │   │   └── glm.rs          # W6-0002
    │   │   ├── checks/
    │   │   │   ├── mod.rs
    │   │   │   ├── process.rs      # W1-0009: ProcessRunner
    │   │   │   ├── property.rs     # W2-0002
    │   │   │   ├── container.rs    # W2-0002
    │   │   │   ├── browser.rs      # W2-0003
    │   │   │   └── external.rs     # W2-0003
    │   │   ├── workspace/
    │   │   │   ├── mod.rs
    │   │   │   ├── direct.rs       # W1-0005
    │   │   │   └── copy_on_write.rs # W4-0001
    │   │   └── policies/           # strategy implementations: one file per port, default plus alternatives
    │   │       ├── mod.rs
    │   │       ├── resources.rs    # W1-0004: PriceWeighted, PurposeBounded
    │   │       ├── belief.rs       # W1-0010: LikelihoodRatioTable plus an experimental model
    │   │       ├── credit.rs       # W1-0010: Confirmed-only, Discriminated
    │   │       ├── intake.rs       # W1-0011: CriteriaExtraction + VoiClarification, NoQuestions
    │   │       ├── planner.rs      # W1-0011; W3-0004 UpfrontDecomposition
    │   │       ├── contribution.rs # W1-0011: OrdinalValue, FixedWorkflow; W6-0004 VocValue
    │   │       ├── method.rs       # W1-0011: FixedMethod; W6-0003 CascadeRouter, LearnedRouter
    │   │       ├── progress.rs     # W1-0012
    │   │       ├── diagnosis.rs    # W1-0012; W3-0007 ModelAssistedDiagnoser
    │   │       ├── escalation.rs   # W1-0012
    │   │       ├── reviewer.rs     # W1-0013 AnyNonProducer; W3-0002 DifferentFamilyComparableStrength
    │   │       ├── context.rs      # W1-0013; W3-0005 handoff
    │   │       ├── narrative.rs    # W1-0013: Narrator, DeterministicReport
    │   │       ├── claim_audit.rs  # W1-0013
    │   │       ├── verification_designer.rs  # W2-0001
    │   │       ├── mutation.rs     # W2-0004
    │   │       ├── team.rs, profile.rs, volunteer.rs, award.rs, dispute.rs   # W3
    │   │       ├── selection.rs    # W4-0003
    │   │       └── reputation.rs, calibration.rs, knowledge_curator.rs, retrieval.rs, trial.rs  # W5
    │   └── tests/
    │       ├── foundation.rs       # W1-0001: real consumer over MemoryJournal
    │       ├── scenarios/          # W1-0014, W3-0008, ...: scenarios on a temporary filesystem
    │       └── fixtures/           # W1-0018: local Codex protocol fixtures
    │
    ├── ymp-storage/                # W1-0016
    │   ├── Cargo.toml              # ymp-domain, ymp-kernel, rusqlite, serde
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── journal.rs          # SqliteJournal
    │   │   └── content.rs          # content-addressed store
    │   └── tests/                  # atomicity, restart, corruption, indeterminate commits
    │
    ├── ymp-tui/                    # W1-0015, Ratatui presentation and control adapter
    │   ├── Cargo.toml              # ymp-runtime, ratatui, crossterm
    │   └── src/
    │       ├── lib.rs
    │       ├── app.rs              # interface state derived only from projections
    │       ├── events.rs           # terminal event loop, responsiveness, terminal restoration
    │       └── screens/mod.rs      # intake, progress, inspection, report; later waves add views
    │
    └── ymp-cli/                    # W1-0015, the ymp executable
        ├── Cargo.toml              # ymp-runtime, ymp-storage, ymp-tui
        └── src/
            ├── main.rs
            ├── lib.rs              # command parsing, exit codes, subcommands
            └── compare.rs          # W3-0009: comparison-runner subcommand
```

## Conventions

- **Ports live in the kernel, implementations in the runtime.** The traits under
  `ymp-kernel/src/ports/` follow the subsections of model section 5, so a
  strategy is found where the document lists it. Under
  `ymp-runtime/src/policies/` one file per port holds the default
  implementation and its alternatives, so the contract test "two materially
  different implementations through the real consumer" reads in one place.
- **The Dispatcher is in the runtime.** The model lists it among kernel
  services, but it calls strategies that live in the runtime; W1-0014 places
  `dispatcher.rs` there. Business rules stay in the owning kernel services;
  the Dispatcher owns call order only (rule D-3).
- **Events are one typed enumeration.** `ymp-kernel/src/events.rs` grows by one
  variant per event of model section 9. Each task adds its variants without
  changing records written earlier, as W1-0001 requires.
- **The interface is its own crate.** `ymp-tui` holds screens and the event
  loop and depends only on `ymp-runtime`; `ymp-cli` composes it with storage
  selection and non-interactive subcommands. Presentation never owns domain
  state or grants authority (model section 3.9), and headless commands do not
  link a terminal UI.
- **Tests sit next to their consumer.** Integration tests live in each crate's
  `tests/`, shared helpers in `tests/support/mod.rs`, filesystem scenarios and
  protocol fixtures only under `ymp-runtime/tests/`.
- **Workspace manifest.** `resolver = "3"`, `edition = "2024"`,
  `rust-version = "1.98.1"`, `publish = false`, `unsafe_code = "forbid"` and
  `clippy::all = "warn"` for the whole workspace, so that
  `cargo clippy -- -D warnings` from `AGENTS.md` is meaningful from the first
  commit.
- **External dependencies of W1-0001** are limited to `sha2` for `Digest` and
  `serde` with `serde_json` for versioned payloads and recoverable policy
  parameters. Every later dependency is added by the task that needs it.

## Consequences to keep in mind

- The `ymp` binary exists as a placeholder with an empty `main` until W1-0015;
  it does nothing. The first real consumer is `DecisionConsumer` composed with
  `MemoryJournal` in the runtime tests. W1-0002 and W1-0014 add `Application` and
  filesystem session scenarios.
- The original skeleton passed the four required checks with zero tests.
  Implementation tasks add focused evidence and the dependencies they actually
  use; each commit must keep the required checks passing.
