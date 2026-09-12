# YMP-112 backend validation

Implementation base: `9ad43dccb29840089a3f351c5651867aa808135d` in the authorized isolated `sibling112` development checkout. The task registry, approved intent, other writers' checkouts and TUI sources are unchanged by this assignment.

## Checks

- `cargo fmt --all --check`: passed; [output](fmt.txt).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed; [output](clippy.txt).
- `cargo test --workspace`: passed (306 tests, including nine board scenarios; final completion 2026-09-12 18:18 UTC); [output](workspace-tests.txt).
- `git diff --check`: passed.

The board suite has nine public-engine integration tests. Native execution is supplied by a scripted backend; coordination calls use the actual local team socket and assignment capabilities. Every fixture configuration pins a small mock model with low effort. No live provider inference, credentials, service or application source copies are involved.

The focused tests cover conflicting claims and a different injected strategy choosing a different winner; stale task, plan and actual membership versions; task distribution, explicit reassignment and historical membership; additions and approach changes preserving responsibilities; forbidden identity, model, membership and contract fields; expired authority; atomic competing claims without issuing invocation authority; and inspection of an admitted uncertain effect before a fresh attempt while preserving the original confirmed sibling decision.

## Discriminating controls

[The recorded negative controls](negative-controls.txt) deliberately broke two production protections and each failed the public consumer test with exit 101:

1. Disabling the execution-loop board consumer resulted in no committed proposal decisions.
2. Removing the prior commitment generation from the task version allowed the second conflicting claim to commit.

The source was restored after each control and the unmodified focused suite passed. Full workspace testing also exposed an intermediate membership-version bug: reordering identical current members during independent review expired otherwise valid proposals. The final digest sorts identity sets while preserving actual membership, eligibility and reviewer changes. The added stale-membership and responsibility-preservation tests pass with that correction.

## Delivery boundary

This record establishes backend behavior and offline verification. It does not claim strategy quality or native accounting guarantees beyond the existing resource contract. A responsibility applies to the next attempt; native work still requires current admission, settings, workspace access and a fresh grant. Plan revisions are intentionally additive and cannot erase accepted work or installed acceptance obligations.

The separately delegated Claude Opus 5 max TUI integration and independent release acceptance remain the parent's responsibility. Shared DTOs and expected view behavior are documented in [board coordination](../../architecture/board-coordination.md).
