# YMP-201 preparation evidence

This is a preparation package, not a native experiment or model-quality result.
The work starts from **399ecc2c40ba2473f2676c482648e664cba9a52e** on
`eval/ymp201-weak-pilot`. The parent retains the study hypothesis, sources,
spending decision, integration and final interpretation.

## Delivered scope

- [Pilot instructions and commands](../../../ymp-evals/weak-pilot/README.md):
  two tasks, each with distinct preparation/measured variants; public-only export;
  deterministic output selection; blind artifact freezing and external scoring.
- [Proposed 12-attempt manifest](proposed-manifest.json): reproducible seed
  `2010914`, source/checker hashes, explicit shared allowance and no actual model,
  usage, elapsed time, acceptance or score observations.
- [Manifest, result and invocation contracts](../../../ymp-evals/weak-pilot/manifest.template.json):
  actual requested/sent/reported settings and usage belong to originating agents
  and invocations. Unknown data stays null; preparation is separate.
- [Negative controls](negative-controls.json): 12 actual incorrect artifacts
  rejected, including both original Python defects. Seven pass public examples
  but fail the external checker. All four synthetic reference submissions pass.
- `ymp-weak-pilot`, an additional bin inside the existing evaluation crate:
  raw-turn observation with production Store admission, current Engine review,
  and a Python stdio fixture through `NativeExecutionBackend`. It neither uses
  the existing scripted task-answer writer nor manufactures acceptance records.

The only Cargo change is `default-run = "ymp-eval-driver"` in that crate, so the
existing `cargo run -p ymp-eval-driver -- ...` command remains unambiguous. No
dependencies, production core/runtime/storage/providers/CLI/UI, intent, task
registry, other branches, installation or release artifacts were changed.

## Verification and audit

[checks.json](checks.json) records exact commands, exit codes and preparation
elapsed time; the per-command logs retain the complete results. The final suite
contains 36 Python tests (10 new pilot tests) and 580 passing Rust tests, with
two existing ignored tests. `cargo fmt --all --check`, workspace Clippy with
`-D warnings`, `cargo test --workspace` and the research-bin debug build pass.
Checks use only `/Users/maggnus/Code/ymp2/target` debug/test outputs.
Tracked workspace-test logs omit trailing empty lines; byte-for-byte captured
copies remain in the corresponding external evidence directories.

[offline-proof.json](offline-proof.json) has 12 passing protocol cases:
independent 1/2/3, group budget, unknown usage, partial usage, provider error,
shared deadline, observed-token overshoot/stop, cooperation 2/3 and native Codex
wire-format validation using a local Python process. The last case observes low
in both `thread/start` and `turn/start`; acknowledged high prevents `turn/start`.
Repeated input/cache/output/reasoning snapshots do not double-count usage.
Error, cancellation and partial-usage records retain observed spend.

The original full test attempt failed because the offline two-second turn timeout
coincided with the provider's two-second process-close wait, masking the actual
effort-mismatch cause. The fixture timeout is now five seconds; separate checks
require both zero `turn/start` calls and the original mismatch cause. The initial
failure is retained in [workspace-tests-initial.log](workspace-tests-initial.log).

Independent inspection of the first passing report then found that a captured
three-profile roster had only two invoked agents. That earlier report and logs
are explicitly named `*-before-roster-audit.*`; they do **not** establish three
active participants. The final cooperation-3 fixture uses two independent visible
protocol tasks, the current scheduler ceiling of two, and eight allowed outer
invocations. The tests now require every captured actor to have an invocation,
exactly N-1 producers, an independent final reviewer, and native contexts bound to
their originating actors. Conflicting writes remain serialized by the current
workspace coordinator. This preparation correction did not change either
measured task's inputs or reveal a measured outcome.

[native-preflight.json](native-preflight.json) is the actual refusal from the
native command: exit 1, `blocked_before_native`, `provider_spawned: false` and
null actual model/usage/time. It contains no authorization bypass. The wrapper
never invoked installed Codex, Claude, npx or a model. The stdio protocol check
executes only its checked-in Python fixture, without credentials or native homes.

[source-binding.json](source-binding.json) binds the prepared source and debug
binary. [raw-evidence-index.json](raw-evidence-index.json) lists hashes and the
explicit external location of all raw JSON/JSONL protocol evidence. Complete
temporary Store metadata also remains there; it is not copied into the repository.
The deterministic command in the pilot README regenerates this evidence without
inference if temporary files have subsequently been removed.

## Acceptance limits and next step

The executable **offline** package is complete. The native command deliberately
implements a preflight refusal, not an enabled measurement runner. The raw proof
adapter uses synthetic config/inputs; enabling measurement still requires binding
an approved manifest and visible task exports to verified native controls and
supervising public selection under the same absolute deadline. No inference call
was used to calibrate tasks, limits or model labels.

Native subagent suppression, native memory/retry suppression, filesystem/context
confinement and a finite in-flight native token ceiling remain unproven. Their
exact existing API limitations are recorded in
[quota-and-boundary.md](../../../ymp-evals/weak-pilot/quota-and-boundary.md).
A cwd is not a filesystem sandbox; a roster is not proof of actual participation;
a bounded outer invocation is not a bounded number of native API requests.
Production acceptance has not been relaxed. YMP-148 is not required by this work.

The current proposal is 8 outer invocations, a shared 120-second deadline,
45 seconds per turn, one application attempt, scheduler ceiling 2 and a 12,000
observed input-plus-output token threshold per outcome. It is neither an approved
quota nor a hard billed-token ceiling. The parent must resolve the recorded native
boundaries, review actual participation requirements, approve the executable
commands and expenditure, and freeze the manifest before any native attempt.

Development-agent usage/cost and total preparation time were not supplied by the
execution service and remain unknown. Command durations in checks.json are actual
preparation observations only. Linux execution, Python 3.9 specifically, the two
pre-existing ignored tests, release packaging and all real-provider checks were
not performed. This work used Python 3.14.3 and rustc 1.89.0 on macOS.
