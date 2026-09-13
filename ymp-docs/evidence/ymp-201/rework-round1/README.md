# YMP-201 round-1 rework

Base candidate: `8a2acafe2aadb0e00b7524b3246f898659f38fd8`. The agreed contract
is copied from documentation commit `858fba3` in [before/review-contract.md](before/review-contract.md).
Original reviewer artifacts, scripts, results and hashes are preserved under
`before/`; neither the primary repository's originals nor
`/tmp/ymp201-consumer-review` were edited. This branch does not import new main
commits. Product crates/dependencies remain the accepted `1c17f4e` bytes.

## Corrections

**C1.** `measurement::inspect` supplies one validity rule for raw and Engine
records. It checks originating assignments, requested/sent configuration,
contradictory acknowledgments, context ownership, complete usage and termination.
Missing reported model/effort remains null with incomplete metadata. Underuse is
a requested/actual roster observation, not extra work or an invalid measurement.
Task outcome and objective artifact score are separate fields. Wrong answers,
missing artifacts, normal negative Engine verdicts, fully accounted budget stops
and deadline during sealing continue to the next matched condition. Calibration
readiness depends on valid measurements, never universal task correctness.
Unknown/partial usage, broken boundaries, extra actors and unverified termination
still stop the phase. The review's provider-error case is explicitly partial.

**C2.** Config per-turn time equals the condition ceiling; each raw request gets
the rounded-up actual remainder of its existing absolute deadline. Independent
candidates run in frozen ordinal order and never reset it. Engine retains its
normal scheduler/acceptance and shares the same outer deadline. Solo makes one
full native tool loop, not extra calls for symmetry. Existing prompt/context,
visible-output and reservation limits retain their distinct units; reservations
do not cap the actual tokens of a started call. Tests cover successful spend above
reservation and work beyond the former short per-call phase limit.

**C3.** [restricted_python.py](../../../../ymp-evals/weak-pilot/restricted_python.py)
is shared by public selection, repair scoring and the direct repair-probe CLI.
macOS sandbox-exec allows only copied candidate/probe files and necessary system/
Python runtime reads. It denies private/peer/native-state/credential files, their
parent-directory contents, network and fork, with a clean environment. Expected
values stay in the trusted observer; only functions/arguments enter the scorer's
child. Correct solutions pass; paired artificial canaries fail to read both
direct and symbolic-link paths. Unsupported sandbox setup fails closed.
The trusted Python parent and candidate inherit one process group supervised by
the existing `ymp_providers::supervisor`; closing its liveness pipe triggers bounded
termination and collection before a deadline result is reported. A candidate's
known-ended failure/timeout is a task outcome, not a broken observer.

## Spending and identity

Approval schema 2 covers one exact manifest/phase once. An atomic marker is
created in the manifest's private persistent `approval_ledger` before outputs or
providers start. It remains on success, failure and interruption. Output cleanup
does not renew authority; a new paid phase needs a new frozen manifest and approval.
No actual approval file or native marker is created during this rework. Scripted
tests exercise the same claim operation on an artificial ledger without owner
impersonation or native calls.

The refreshed proposals pin the native binary, native wrapper, Python launcher,
actual framework interpreter, OS sandbox executable and restricted wrapper hashes.
The parent still owns the quota decision. Proposed calibration remains six
preparation outcomes at 80,000 observed tokens/12 outer calls/480 seconds each;
conditional pilot remains twelve outcomes at 160,000/16/900. A call may use the
remaining whole-condition time. These are administrative unapproved limits, not
scientific estimates or a hard billed-token ceiling.

## Verification records

Final results: **643 Rust tests passed**, two existing tests ignored; **62 Python
tests passed** (36 fixture, 16 native-envelope and 10 candidate-boundary checks).
Format and strict workspace Clippy passed. The full 12-cell protocol run and
nine bounded outcome controls are retained in [after-controls.json](after-controls.json).
The valid-negative cases continue and permit calibration completion; unknown
usage, the explicitly partial provider error, and a broken envelope stop.
The original negative `score` behavior required no correction.

- `focused-rust.log`: measurement/approval units and actual consumer integration
  controls, including later execution after valid negative outcomes, shared time,
  missing acknowledgments across all conditions, and unknown-spend stops.
- [c3/README.md](c3/README.md): paired selection/scoring controls and preserved
  candidate-only input/permission evidence.
- `final-checks.json`: one required final fmt/strict Clippy/workspace chain plus
  Python suites on the final source.
- `native-envelope-final.json`: final-wrapper native configuration/permission/MCP
  check with zero native `turn/start` calls.
- `final-binding.json`: exact source, executable/wrapper/interpreter and manifest
  hashes, with accepted product-object comparison.

[paired-canary-final.json](paired-canary-final.json) reruns selection and scoring
against the final restricted wrapper and records the refused direct, symlink and
parent reads. [approval-refusals.json](approval-refusals.json) records that both
new native proposals pass concrete validation and stop only at missing approval.
Log copies omit trailing empty lines only; raw copies remain in the indexed
external run directory. The original reviewer patch is retained byte-for-byte,
including its original whitespace. An initial focused test compile omission in
synthetic session setup was fixed before the successful focused/final checks.

## Reviewable commands and proposals

- [Calibration manifest](calibration-manifest.proposal.json): six preparation
  outcomes, shared 480 seconds per condition, 80,000 observed tokens and 12 outer
  calls. The native call may use all remaining condition time.
- [Pilot manifest](pilot-manifest.proposal.json): twelve measured outcomes,
  shared 900 seconds, 160,000 observed tokens and 16 outer calls, conditional only
  on trustworthy native calibration measurements. Wrong calibration answers do
  not close this gate. Neither proposal is approved.

From this checkout, with the frozen debug binary:

```sh
/Users/maggnus/Code/ymp2/target/debug/ymp-weak-pilot native --manifest ymp-docs/evidence/ymp-201/rework-round1/calibration-manifest.proposal.json --approval /absolute/calibration-approval.json
/Users/maggnus/Code/ymp2/target/debug/ymp-weak-pilot native --manifest ymp-docs/evidence/ymp-201/rework-round1/pilot-manifest.proposal.json --approval /absolute/pilot-approval.json --calibration-report /private/tmp/ymp201-r1-calibration-controller/run.json
```

Those commands are for a later authorized phase, not instructions to run now.
The manifests bind source commit, interpreter, wrappers and executable; a rebuilt
binary with a different digest requires a newly reviewed freeze. The approval
format is documented in the [consumer contract](../../../../ymp-evals/weak-pilot/consumer-v2.md).

No real model trial, calibration, quota use, live application/session access,
credential copying, installation, task-register change or main-branch edit is
part of the work. macOS is the verified candidate-execution platform; unsupported
platforms refuse. Real model quality, native costs and in-flight billed-token
overshoot remain unmeasured. Preparation-agent expenditure remains separate and
unknown where the execution service supplied no accounting.
