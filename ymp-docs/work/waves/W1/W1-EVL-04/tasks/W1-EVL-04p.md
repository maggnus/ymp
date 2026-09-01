---
id: W1-EVL-04p
kind: task
wave: W1
card: W1-EVL-04
state: accepted
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04i, W1-EVL-04o]
blocks: [W1-EVL-04q]
created_at: 2026-09-01T18:19:05+08:00
updated_at: 2026-09-01T19:34:21+08:00
started_at: 2026-09-01T18:58:42+08:00
accepted_at: 2026-09-01T19:34:21+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/d0a49e481aea9e8ad9f2cc15a8dc260d9300af24
closure_commit: https://github.com/maggnus/ymp/commit/777dfd597c89f9de3f194e326a13d819bc193231
evidence: ["[777dfd5](https://github.com/maggnus/ymp/commit/777dfd597c89f9de3f194e326a13d819bc193231)"]
duration_minutes: 32
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 2
escalation_decision:
---

# W1-EVL-04p — Runtime compatibility is behavioral, not version-pinned

## Outcome

The Codex driver and supervisor admit an installed runtime by a versioned behavioral compatibility
contract over its measured help, capabilities, App Server schema, events, usage, resume and process
lifecycle; the observed CLI version and executable digest are evidence, never a user-managed selector
or a hard-coded acceptance condition.

## Scope

### In

- Replace exact-version equality in `ymp-rust/crates/ymp-runtime-codex/src/lib.rs` with one canonical
  `CODEX_COMPATIBILITY_CONTRACT` and digest covering the accepted executable behavior measured by
  W1-EVL-04i. Discovery records the observed version string and executable digest after those
  behavioral checks pass.
- Rename version-specific internal helpers/diagnostics to behavioral names. A newer or older CLI with
  the same required surface is compatible; any CLI, including the former reference version, with a
  changed flag, feature state, App Server/tool/event/usage schema, resume/cancellation or descendant
  behavior is incompatible.
- Update only the narrow runtime projection and exact compatibility checks in
  `ymp-rust/crates/ymp-runtime-supervisor/src/lib.rs` so the supervisor binds the compatibility
  contract digest plus observed version/executable digest, not a predetermined version number.
- Add explicit `compatibility_contract_digest` and `executable_digest` fields to
  `ToolHostProbeRuntimeIdentity` in `ymp-rust/crates/ymp-runtime-api/src/lib.rs`; overloading
  `profile`, `driver_version` or another string is forbidden. The accepted W1-EVL-04o transport
  identity remains unchanged.
- Exclusive write zone: those three production files, optional focused additions under their
  existing tests, mechanically forced identity literals only in
  `ymp-application/src/tool_host_probe.rs`, `ymp-cli/src/internal.rs` and
  `ymp-runtime-supervisor/tests/tool_host_probe.rs`, plus mechanically forced changes only in the
  three package manifests and `ymp-rust/Cargo.lock`.

### Out

- No-touch: product CLI/TUI behavior, Application logic, corpus/admission manifests, Claude, prompts,
  model/provider selection, budgets, research/calibration history, deployment and real model/network
  calls. W1-EVL-04q owns product/admission consumers.

## Acceptance

- [x] Two fake executables with different version strings but byte-for-byte equivalent required
      behavior both pass and yield the same compatibility-contract digest while recording distinct
      observed version and executable digests.
- [x] A fake executable reporting the former reference version but changing any required flag,
      feature, App Server/tool/event/usage shape, resume identity, cancellation or descendant
      termination fails for its exact behavioral reason before an accepted task starts.
- [x] The supervisor projection and launch evidence bind contract digest, observed version and
      executable digest. Missing/stale contract digest, behavior drift or executable TOCTOU fails;
      version-string difference alone does not.
- [x] No public/internal CLI option, environment setting, prompt or TUI action lets the user choose,
      pin, downgrade or acknowledge a Codex version. Compatible installed execution is automatic;
      incompatible behavior produces one product-owned diagnostic.
- [x] Focused runtime/supervisor tests, a mutation restoring exact-version equality, strict Clippy,
      formatting and `git diff --check` pass without model/network/money calls or warnings.

## Current state

Accepted and integrated through
[777dfd5](https://github.com/maggnus/ymp/commit/777dfd597c89f9de3f194e326a13d819bc193231).
Codex is now admitted by the measured behavioral contract; observed version and executable digest
remain evidence, and exact 0.151 remains only the historical reference.

## Next action

Run W1-EVL-04q to add the v2 product/admission consumer without rewriting accepted v1 evidence.

## Guardrails

- Compatibility means observed behavior, not semantic-version ordering or optimistic proximity.
- Exact observed version and executable digest remain durable experimental evidence.
- No automatic download, upgrade prompt or user-facing version workflow is introduced.

## Findings

- Owner correction: exact 0.151 support is a temporary calibration point, not a product requirement.
- R1 contract review required explicit compatibility-contract and executable digest fields in
  runtime-api; the task is serialized after W1-EVL-04o because both own that identity type.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

- R1(5/10) RETURN 01/09 19:25 — driver behavior was measured but supervisor and Application still
  compared `cli_version` → the author agreed → all version equality was removed while contract and
  executable digests remained authoritative
- R2(9/10) ACCEPT 01/09 19:33 — an external isolated supervisor scenario accepts a differing version
  and records it, while executable replacement and old-version behavior drift fail before start;
  Git and real user state remain unchanged

## Closure

### Accepted outcome

`CODEX_COMPATIBILITY_CONTRACT` now canonically identifies the measured help, capabilities, App
Server schema and lifecycle behavior. Driver and supervisor admit by its digest and actual executable
bytes; `cli_version` is required to be nonempty and is persisted only as observed evidence. No
version selector, update prompt, download path or user acknowledgement was introduced.

### Residuals

The accepted v1 product/admission fixtures still encode exact 0.151 historical authority. They
cannot open the new behavioral gate until additive W1-EVL-04q v2 is accepted.

### Evidence

- [777dfd5](https://github.com/maggnus/ymp/commit/777dfd597c89f9de3f194e326a13d819bc193231)
  — integrated final tree corresponding to candidate sequence
  [76c1ca1](https://github.com/maggnus/ymp/commit/76c1ca1e39be5ea193e387fb7c65e1788d11d98a) and
  [d0a49e4](https://github.com/maggnus/ymp/commit/d0a49e481aea9e8ad9f2cc15a8dc260d9300af24).
