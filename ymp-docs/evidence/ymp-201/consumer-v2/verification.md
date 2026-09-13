# YMP-201 connected consumer verification

The native consumer is connected; it no longer has a permanent refusal branch.
The checked-in proposals pass file/config/model/catalog/permission/source checks
and stop at the missing external owner approval, before a provider starts.
No real model calibration or measured trial has been performed.

## Source and scope

- Preparation commit: `15529e6bba9995964507d8ba7c28c13870968382`.
- Native consumer implementation: `f122655`.
- Accepted P0 base: `1c17f4e447b839e20f8efbdee0900963a5d8fcad` (accepted source
  `c28f501`, backend `743543f`).
- Normal merge preserving both lines: `7c87117`.
- Final consumer source: `0ac1650`, including combined-source checks, protected
  prior-study roots, opaque random blind IDs and validation before authorization.

The product crates, SDK bridges and root Cargo manifests are byte-identical to
the accepted P0 base. Only the permitted evaluation areas changed. The original
task variants and version-1 protocols remain intact. Native and scripted runs
use one manifest consumer; no integration into main or installation occurred.

## What was exercised

The complete local protocol run covers both tasks and all six conditions through
visible export, the real provider boundary, production Store/Engine accounting,
public-only selection, blind sealing, result records and external scoring.
All 12 artifacts pass their independent checks. The cooperating pair makes six
outer calls using two actual actors; the triple makes eight using three. Each
triple has two producers and an independent final reviewer. Solo/independent
conditions use one/two/three fresh isolated contexts respectively.

The fixture reports 100 synthetic input-plus-output units per call; duplicate
events still count once. These units and the fixture timings are not used as
native quota estimates. All provider events and raw Store traces remain in the
indexed private controller bundle. Real native authentication is never used by
the scripted executable, and only the checked-in `fixture-*` transport is allowed
in scripted mode.

Negative integration controls exercise absent approval, mode/source/executable
substitution, unknown usage, a shared deadline and a provider error. Later cells
remain `not_started` after interruption. Received usage survives errors and
timeouts. Blind identifiers are random independently of the condition-order seed.
The external scorer receives no condition or model label and cannot select a
candidate using hidden answers.

Real **inference-free** Codex 0.154.0 checks separately establish the interpreted
feature flags, low, cleared inherited instructions, named read/write access,
denied hidden/peer/native-state files, and inherited-MCP suppression. Only the
explicit current ymp endpoint starts. A protected parent with a more-specific
current-workspace grant is verified on artificial files. Native hosts retain
their normal state/authentication; model commands cannot read that state.
See [native-controls.md](native-controls.md) and its retained positive/negative
canary reports. No `turn/start` was sent to the installed native provider.

## Validation records

[final-checks.json](final-checks.json) lists the final commands and actual
preparation durations. Format and workspace Clippy pass; the full workspace has
631 passing tests and two existing ignored tests. The Python suites pass 36
fixture tests and 16 envelope tests. The complete consumer integration tests are
included in the Rust suite.

The first combined run exposed a test false positive: a check for the generic
substring `private/` matched macOS's `/private/var/...` temporary directory. It
now checks the exact hidden-fixture path and also requires the raw prompt to equal
the frozen task prompt. That initial failed log is preserved rather than silently
replaced. No production correction was needed.

[scripted-e2e-receipt.json](scripted-e2e-receipt.json),
[scripted-e2e-run.json](scripted-e2e-run.json) and the evidence index bind the
retained standalone run. [owner-approval-refusals.json](owner-approval-refusals.json)
records the two actual native-command refusals after all manifest checks passed.

## Concrete proposals and remaining decision

- [Calibration manifest](calibration-manifest.proposal.json): six preparation
  outcomes, up to 12 outer calls and 80,000 observed tokens per outcome,
  a shared 480-second deadline and 120 seconds per turn.
- [Conditional pilot manifest](pilot-manifest.proposal.json): 12 measured
  outcomes, up to 16 outer calls and 160,000 observed tokens per outcome,
  a shared 900-second deadline and 180 seconds per turn. It binds the exact
  calibration manifest and cannot consume a scripted calibration report.

Both use explicit low, one application attempt and scheduling ceiling two; all
review/selection/coordination belongs to the same condition allowance. These are
administrative proposals, not scientifically justified or allocated quotas.
Currency cost and a hard billed-token ceiling remain unknown; unfinished work may
overshoot an observed threshold. A failed/interrupted calibration blocks the pilot
and never raises its allowance automatically.

The parent should independently accept the package, then request one concrete
owner decision covering these exact manifests and their conditional spending
limits. No completed owner approval file is supplied or embedded in agent
metadata. Native model quality, actual calibration cost/latency, Linux enforcement
and the two ignored tests remain unverified. Preparation-agent usage is separate
from experiment usage and was not supplied by the execution service.
