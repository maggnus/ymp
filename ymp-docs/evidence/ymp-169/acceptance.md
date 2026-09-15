# YMP-169: independent migration plan acceptance

The requested planning deliverable is complete. The owner requested a separate
migration plan after the architecture appraisal; implementation, installation,
live data migration and native experiments were not part of this assignment.

## Accepted document and baseline

- [Independent plan](../../architecture/v3/migration-plan-kernel-transition.md).
- Initial independently reviewed version SHA-256: `8e46bd429565dc237de6444047bc390c5e6e7e1cf69353ec0502d5f65e61767d`.
- Previous editorial version SHA-256: `d6b9b91b9209b9825f6d5618bab6095c4a4779cab00d8af910100a9739d0528a`.
- Current revised plan SHA-256: `4c2f257907275d20fc2a4d4316392fa718c13ebf97b59d9345e62afe083b7632`.
- [Optional test inventory](../../architecture/v3/migration-test-inventory.md), SHA-256 `edef98a702dfb81a9ee5cbdcd1125a5fc70aef7f793131f1ce9c641508a171e2`.
- Product source: `aa98c242103416c978f441826fdad169cc4c7f64` (`0.4.6` workspace).
- The target proposal and `intent.md` remain unchanged. `AGENTS.md` now records
  the owner's 2026-09-15 permission to delete any existing tests during migration;
  its reviewed SHA-256 is `f4b9740e488d3fff635148bb6bb21c4b952175e9508440b293449512de8cb501`.
- Existing task states, dependencies and delivery order are unchanged. Only the
  YMP-169 design task and its generated register views were added/updated.

## Planning acceptance criteria

| Registered criterion | Verdict and evidence |
| --- | --- |
| Standalone English plan and baseline | Pass: sections 1-2; section 9 records contract decisions as standalone prose. |
| Executable dependency and authority direction | Pass as a design: sections 3-5; independent review accepted acyclic dependencies, atomic admission, workspace/termination ownership and versioned evidence semantics. |
| Distinct phases, outcomes, dependencies and retirement | Pass: P0-P5 preserve the product before P6/P7 change coordination; J1/I1/Q1/E1/R1 have explicit prerequisites; K1-K10 and supported legacy continuation conditions govern retirement. |
| Existing scope, compatibility, intent and next assignment | Pass: sections 8-11 retain existing task ownership, paused work, native restrictions and all five intent goals; P1 is the first bounded proposed implementation. |
| Independent review and document/repository checks | Pass: exact document independently reviewed with no material blocking finding; local references and generated register checked; required Rust checks passed on unchanged product code. |
| Target-stage mapping, effort ranges and optional test removal | Pass: section 8.1 maps all ten stages with closure limits; section 6.1 labels effort assumptions and uncertainty; section 10.1 and the appendix allow any test deletion without retention, replacement, audit or approval prerequisites. Independently reviewed and parent-verified. |

These are verdicts on the plan. K1-K10 in the document are future migration
criteria; this assignment does not claim their new-kernel implementations exist.

## Independent review and required checks

Reviewer: Paseo fork `ac4711a1-1e2d-43a4-8dc9-ba1ce0bf66c2`, `gpt-5.6-sol`,
requested thinking `high`. Parent authored the plan, read the final review and
matched its document hash. Reviewer did not modify repository files.

The [full English review and command logs](/Users/maggnus/ymp-research/architecture-v3-2026-09-15/plan-validation/review-and-checks.md)
are retained outside the repository. Parent checked the logs and source binding.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace` | Exit 0; 666 passed, 0 failed, 2 explicitly ignored |
| `python3 ymp-docs/tasks/manage.py check` | Valid register, intent coverage, dependencies and generated views |

The independently reviewed version's 30 local document/source references resolved.
The previous editorial version had 29. The current revised plan has 31 and the
inventory has 83, all resolved, including YMP-169's generated task anchor.
Product Rust/bridge/evaluation sources remain unchanged.
The required Rust commands were not repeated for final documentation/status-only
updates; they cannot establish future migration correctness.

## Editorial revision

On 2026-09-15 the owner requested removal of references to the source plan and
the comparison table. The document now stands alone; section 9 preserves the
technical decisions as prose. Phase dependencies, K1-K10 and implementation scope
are unchanged. Parent checked the requested removals and all current links. The
independent review and Rust results above remain bound to their recorded version;
they were not repeated for this editorial change.

## Owner clarification and planning refinements

The owner subsequently requested useful refinements and clarified that **any
existing tests may be deleted if that simplifies migration**, including whole
suites. This is an implementation option, not a command to delete tests in this
documentation assignment. The plan and project instructions now impose no
mandatory retained count, protected category, equivalent replacement, exhaustive
audit or additional approval for that option. Product requirements and runtime
validation remain separate from the lifetime of a particular test suite.

Added explicit mapping to all ten target build stages, preliminary engineering-day
ranges with low confidence and stated exclusions, and a file-level test inventory.
Parent and independent scans agree on all 77 file/count pairs: 668 Rust test
attributes overall, including 212 in 27 runtime files. Counts include ignored tests
and Tokio attributes with arguments; they are not new execution results or a
semantic classification of every test function.

The same Sol high fork produced the independent
[inventory](</Users/maggnus/ymp-research/architecture-v3-2026-09-15/test-inventory/test-inventory.md>)
and a [bounded revision review](</Users/maggnus/ymp-research/architecture-v3-2026-09-15/test-inventory/revision-review.md>).
The review found no concrete contradiction in the permission, target-stage mapping,
effort assumptions or standalone scope. Parent matched all three reviewed file
hashes and checked current links and task metadata. No code, tests or target-model
content was changed, and no build, test execution or native inference was repeated
for this documentation revision.

## Unverified and next step

No kernel implementation was created or compiled. Legacy translation, live data
migration, native provider execution, Linux builds and comparative quality,
resource, timing or learning improvements remain untested. The two existing
ignored tests were not executed.

Next proposed action is owner selection of the implementation scope, followed by
P0's contract resolution and the bounded P1 seam. The plan and task completion do
not themselves approve or start those phases.
