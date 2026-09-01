---
id: W1-EVL-04n
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04l, W1-EVL-04o, W1-EVL-04p, W1-EVL-04q]
blocks: [W1-EVL-04r]
created_at: 2026-09-01T17:17:30+08:00
updated_at: 2026-09-01T21:14:37+08:00
started_at: 2026-09-01T20:13:34+08:00
accepted_at: 2026-09-01T21:14:37+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/f914f2b92564b6143fbf420cea3b995f610e3452
closure_commit: https://github.com/maggnus/ymp/commit/ee96f70fded65303fcc70c90d5b53eed4ae6db98
evidence: ["[ee96f70](https://github.com/maggnus/ymp/commit/ee96f70fded65303fcc70c90d5b53eed4ae6db98)"]
duration_minutes: 56
blocker:
pause_reason:
return_trigger: W1-EVL-04r and W1-EVL-04s accepted
deliberate_partial: true
review_rounds: 2
escalation_decision:
---

# W1-EVL-04n — Compatible Codex runtime performs one attested tool-host probe

## Outcome

An installed Codex runtime whose observed behavior satisfies the W1-EVL-04p compatibility contract
can execute one no-task-output probe through the W1-EVL-04o two-tool MCP transport and return an
explicitly untrusted trace that only the W1-EVL-04l Application controller can attest. Its exact
observed version and executable digest are evidence, not an acceptance selector.

## Scope

### In

- Implement `RuntimeDriver::tool_host_probe_identity` and `start_tool_host_probe` for the exact
  accepted behavioral compatibility contract in `ymp-rust/crates/ymp-runtime-codex/src/lib.rs`,
  using the fixed
  W1-EVL-04o server, `workspace_write` then `workspace_read`, the controller-supplied path and nonce,
  and a fixed compatibility directive that is not a user task or experimental arm. Nonempty ordinary
  output remains an error rather than hidden evidence.
- Immediately before `start_tool_host_probe` spawns the MCP child, independently rehash its actual
  executable bytes and recompute the W1-EVL-04o launch configuration/root identity; compare them to
  the controller reservation. Any post-reservation replacement, path/config/root change or symlink
  fails before the provider request and before an attestation can exist.
- Replace the fake-only guard in `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs` with an exact
  allowlist of `Fake` for deterministic tests and a Codex runtime carrying the accepted behavioral
  contract plus exact observed runtime/probe transport identity. A public direct caller may receive
  only the existing `UntrustedRuntimeTrace`; it gains no controller read-back, private attestation or
  `model_ready` authority.
- Focused tests may be added only under
  `ymp-rust/crates/ymp-runtime-{codex,supervisor}/tests/**`. One optional test-only
  `ymp-rust/crates/ymp-cli/tests/live_tool_host_probe.rs` may create the isolated Application store
  and spawn `CARGO_BIN_EXE_ymp internal tool-host-probe`; it may not change CLI production code.
  Mechanically forced changes are allowed only in those package manifests and `ymp-rust/Cargo.lock`.
- A narrow typed change in `ymp-runtime-api/src/lib.rs` may replace the lossy
  `ToolHostProbeError::RuntimeFailed { detail }` with structured failure evidence containing
  `RuntimeFailureKind`, complete terminal `Usage`, optional bounded `DiagnosticSummary`, terminal
  event id and sequence. Its display may expose only those sanitized fields, never raw stderr.
- One admitted live probe uses the additive W1-EVL-04q v2 manifest and separate stage-two budget: exactly
  one model request, no task/arm output, no board/recruitment/candidate tool, and one ordered
  write/read exchange in a fresh disposable root. The W1-EVL-04l controller independently reads and
  attests it before the model gate can open.
- The W1-EVL-04q manifest binds the compatibility-contract and collaboration MCP digests. The opaque attestation
  independently binds the probe tool-schema digest and the W1-EVL-04o `probe_transport_digest`; no
  manifest byte changes in this task.

### Out

- No-touch: all other `ymp-runtime-api` behavior, `ymp-application/**`, `ymp-agent-mcp/**`, `ymp-cli/src/**`,
  `ymp-corpus/**`, admission
  manifest/digest, Claude runtime, TUI, research and calibration records, task prompts, experimental
  arms, primary/development seeds and budgets, real user HOME, arbitrary network tools and any model
  call beyond the single frozen probe.

## Acceptance

- [x] A behaviorally compatible fake Codex with any observed version plus the probe run exposes only
      W1-EVL-04o workspace write and read, carries compatibility-contract, observed
      version/executable, route/profile/driver, probe schema and transport digests, writes then reads
      the supplied path, reports complete usage/cost/terminal evidence and yields a W1-EVL-04l
      attestation with `model_calls=1` and `model_ready=false` until W1-EVL-04m consumes it.
- [x] A direct caller can obtain at most `UntrustedRuntimeTrace`; presenting it or a raw request to
      Application/04m creates no attestation. Wrong/missing compatibility contract, behavior drift,
      route/profile/schema/transport mismatch, absolute/traversing path, missing/reordered tool event, output, extra effect,
      timeout/cancellation, incomplete usage or budget overflow returns a typed refusal.
- [ ] The live run starts only after zero-model readiness and all deterministic mutations pass. It
      uses one fresh short root with separate project, `HOME`, `YMP_HOME`, `TMPDIR`, build and export;
      it performs exactly one provider request within the frozen stage-two vector and is never
      continued or selectively retried after any failure.
- [x] A schema-identical child measured correctly and then replaced or reconfigured before spawn is
      refused by the immediate runtime/supervisor recheck with `model_calls=0`, no attestation and no
      reservation refund.
- [x] Every terminal runtime failure preserves failure kind, terminal sequence/id, complete `Usage`
      and optional diagnostic digest/size/truncation through the public error and CLI display. Raw
      stderr is absent; missing statistics are explicit rather than silently replaced with zeros.
- [ ] The controller proves the destination absent before launch and its independent read-back equals
      the nonce after the ordered tool events. A `Fake` trace, schema-identical substitute server or
      mismatched transport digest may test mechanics but can never satisfy the live admission
      evidence consumed by W1-EVL-04m.
- [x] Focused Codex/supervisor/controller-bound tests, strict affected-package Clippy, formatting and
      `git diff --check` pass. The real repository, current directory and real `~/.ymp` remain
      byte-identical; the disposable root is removed after evidence capture.

## Current state

Implementation accepted and integrated through
[ee96f70](https://github.com/maggnus/ymp/commit/ee96f70fded65303fcc70c90d5b53eed4ae6db98).
The first authorized live attempt ended `ProcessExit` after 47.65 s without trace/attestation; it is
`infrastructure-invalid / failure phase indeterminate`, and the model gate remains closed.

## Next action

Implement W1-EVL-04r without model calls, then execute the separately budgeted W1-EVL-04s once; do
not retry the 04n attempt.

## Guardrails

- One accepted profile, one purpose, one request. This is not a generic live-probe API.
- Infrastructure reachability is not communication, listening, task value or self-organization.
- The live probe is an admitted evaluation expense, not part of any matched-budget arm.

## Findings

- Scientific peer review found that W1-EVL-04j plus W1-EVL-04l could otherwise never produce a
  positive real-profile attestation. This task is the minimal bridge rather than a relaxation of the
  fake seam for arbitrary drivers.
- R1 contract review found no controller marker in the public trace API, no private two-tool MCP
  server, no dedicated probe launch mode and ambiguity between collaboration and probe schema
  digests. The corrected graph assigns the exact transport to W1-EVL-04o, keeps public traces
  untrusted and binds both schema domains without changing the manifest.
- Owner correction removed exact CLI version as product authority. W1-EVL-04p admits observed
  behavior and W1-EVL-04q preserves exact version/digest only as reproducibility evidence.
- W1-EVL-04o R1 review assigned the final expected-vs-actual executable/config/root recheck here,
  because only this node owns the real pre-spawn boundary.
- Final pre-dispatch review confirmed that the only missing code is exactly this task's two-file
  outcome: Codex probe methods and removal of the Fake-only supervisor guard. CTO accepted dispatch;
  no additional dependency or write-zone expansion was identified.
- The live consumer check must execute the actual `ymp` binary so its private MCP child is reachable;
  the optional CLI integration test owns only disposable-store bootstrap and process observation.
- The first and only live attempt stopped with `ProcessExit` after 47.65 s and produced no
  attestation. R1 code review found `RuntimeEventKind::Failed` discarded its `Usage` and
  `DiagnosticSummary`; this must be fixed deterministically before any separately authorized run.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(6/10) RETURN 01/09 21:00 — deterministic launch boundary was correct, but terminal
  `DiagnosticSummary` and `Usage` were discarded → the author agreed → a structured safe failure
  variant now preserves kind, usage, event identity and diagnostic digest without raw stderr
- R2(9/10) ACCEPT 01/09 21:13 — code accepted with live STOP: deterministic substitute-child falsifier fails before
  start with `model_calls=0`, structured failures survive exactly and launch code is unchanged →
  implementation is safe to integrate while live admission remains explicitly unaccepted

## Closure

### Accepted outcome

Codex now implements the bounded probe session through the private two-tool MCP child; supervisor
admits only behaviorally compatible Codex or deterministic Fake, reconstructs exact identities and
rehashes child/configuration/root immediately before spawn. Direct calls yield only an untrusted
trace. Terminal failures preserve complete sanitized evidence. All deterministic checks and the
mutation removing the final rehash passed their expected positive/negative halves.

### Residuals

The only 04n live attempt ended `ProcessExit` after 47.65 s. Provider request, usage and failure phase
could not be established from the pre-fix error; no tool event, controller read-back, handle or
attestation exists and `model_ready=false`. W1-EVL-04r must persist phase-localized failure evidence;
W1-EVL-04s alone may spend a fresh budget on one new attempt.

### Evidence

- [ee96f70](https://github.com/maggnus/ymp/commit/ee96f70fded65303fcc70c90d5b53eed4ae6db98)
  — integrated final implementation corresponding to candidate sequence
  [2ab84f4](https://github.com/maggnus/ymp/commit/2ab84f4449d44ea103e0bca820929daaab9ab057) and
  [f914f2b](https://github.com/maggnus/ymp/commit/f914f2b92564b6143fbf420cea3b995f610e3452).
- Live attempt fingerprints: `ymp=f3707ba48d784047c992c27f49c921f3c208a89f23225a67bd7dbf70d915a9d7`,
  `codex=98491713ffb196061003ee148636e743997cc31d76144ba7c53462269896891d`,
  `admission-v2=d354f20c8482cd5df7e33fab70dcd267befb621ef09647d430dc40f3924b8ea2`.
