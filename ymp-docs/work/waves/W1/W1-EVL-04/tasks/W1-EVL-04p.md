---
id: W1-EVL-04p
kind: task
wave: W1
card: W1-EVL-04
state: ready
risk: critical
maturity: BUILD
relation: required
depends_on: [W1-EVL-04i]
blocks: [W1-EVL-04q]
created_at: 2026-09-01T18:19:05+08:00
updated_at: 2026-09-01T18:19:05+08:00
started_at:
accepted_at:
candidate_commit:
closure_commit:
evidence:
duration_minutes: 0
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
review_rounds: 0
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
- Exclusive write zone: those two production files, optional focused additions under their existing
  tests, and mechanically forced changes only in the two package manifests and `ymp-rust/Cargo.lock`.

### Out

- No-touch: product CLI/TUI, Application, corpus/admission manifests, runtime-api, Claude, prompts,
  model/provider selection, budgets, research/calibration history, deployment and real model/network
  calls. W1-EVL-04q owns product/admission consumers.

## Acceptance

- [ ] Two fake executables with different version strings but byte-for-byte equivalent required
      behavior both pass and yield the same compatibility-contract digest while recording distinct
      observed version and executable digests.
- [ ] A fake executable reporting the former reference version but changing any required flag,
      feature, App Server/tool/event/usage shape, resume identity, cancellation or descendant
      termination fails for its exact behavioral reason before an accepted task starts.
- [ ] The supervisor projection and launch evidence bind contract digest, observed version and
      executable digest. Missing/stale contract digest, behavior drift or executable TOCTOU fails;
      version-string difference alone does not.
- [ ] No public/internal CLI option, environment setting, prompt or TUI action lets the user choose,
      pin, downgrade or acknowledge a Codex version. Compatible installed execution is automatic;
      incompatible behavior produces one product-owned diagnostic.
- [ ] Focused runtime/supervisor tests, a mutation restoring exact-version equality, strict Clippy,
      formatting and `git diff --check` pass without model/network/money calls or warnings.

## Current state

W1-EVL-04i/04k made Codex 0.151 a reproducible reference but also made its version string an
acceptance gate. That is useful evidence and wrong product policy: users must not manage versions,
and ymp cannot maintain one hard-coded CLI release. The accepted behavioral measurements already
provide the stronger compatibility boundary needed here.

## Next action

Run a Critical contract check, then dispatch one Sol xhigh builder independently of active W1-EVL-04o
because their write zones do not overlap.

## Guardrails

- Compatibility means observed behavior, not semantic-version ordering or optimistic proximity.
- Exact observed version and executable digest remain durable experimental evidence.
- No automatic download, upgrade prompt or user-facing version workflow is introduced.

## Findings

- Owner correction: exact 0.151 support is a temporary calibration point, not a product requirement.

## Review rounds

One line per round of the convergence loop, written by the CTO from the two roles' reports:
`- R1(7/10) RETURN <dd/mm hh:mm> — <finding> → <the author's evidenced answer> → <what changed>`. The
marker carries the reviewer's ten-point score and the local moment of the verdict. After an
escalation, one `- CTO <decision> <dd/mm hh:mm> — <reason>` line records what was decided. The review
dialogue itself stays in the reports and the evidence package.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

What was actually accepted.

### Residuals

Honestly retained limitations, each with an exact return trigger. Empty when there are none.

### Evidence

- Commit, evidence package, or durable document of record, each as a Markdown link.
