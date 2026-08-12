---
id: W1-APP-02g
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: []
created_at: 2026-08-12T09:42:00+08:00
updated_at: 2026-08-12T11:18:18+08:00
started_at: 2026-08-12T10:28:00+08:00
accepted_at: 2026-08-12T11:18:18+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/7791162f2f561392bce689a518717b0af2eecb4d
closure_commit: https://github.com/maggnus/ymp/commit/973a6e331f4577a894ab95ee325efa4a03005510
evidence: ["[7791162](https://github.com/maggnus/ymp/commit/7791162f2f561392bce689a518717b0af2eecb4d)", "[973a6e3](https://github.com/maggnus/ymp/commit/973a6e331f4577a894ab95ee325efa4a03005510)"]
duration_minutes: 50
blocker:
pause_reason:
return_trigger:
deliberate_partial: false
---

# W1-APP-02g — Verification evidence binds the exact runtime environment

## Outcome

Every accepted verification record identifies the exact environment object used by the verifier,
and replay refuses evidence whose environment digest is absent or mismatched.

## Scope

### In

- Durable verification-record fields, environment object binding, compatibility handling, and
  verifier/application checks that enforce the binding.

### Out

- Runtime event ordering, participant-session resume, TUI layout, and unrelated schema revisions.

## Acceptance

- [x] A newly accepted record carries the exact environment digest consumed by verification and
  reproduces from clean immutable inputs.
- [x] Missing, substituted, or mismatched environment data is rejected as a typed infrastructure
  failure rather than accepted evidence.
- [x] Compatibility fixtures cover the deliberate schema decision for existing version-1 data.

## Current state

Accepted. Evidence schema version 2 binds each verification to the exact immutable environment
object, while `StoredVerificationEvidence` keeps recovered data outside the API that records new
verification results.

## Next action

Proceed with the remaining required W1-APP-02 tasks.

## Guardrails

- Preserve content-addressed immutable evidence and fail closed on ambiguous legacy data.
- Any durable schema change must update its compatibility fixtures and `ymp-rust/SCHEMA.md`.

## Findings

None.

## Closure

Filled when the task is accepted. Until then this section stays as written.

### Accepted outcome

Application stores the environment bytes before verification, supplies their exact immutable path
and digest to Verifier, verifies the object again before recording, and checks both evidence and
environment during recovery and export. Independent review confirmed the version-2 path and proved
that recovered data cannot be passed to `record_verification` or converted into verifier-issued
evidence through the public API.

### Residuals

None.

### Evidence

- [Reviewed correction](https://github.com/maggnus/ymp/commit/7791162f2f561392bce689a518717b0af2eecb4d).
- [Integration commit](https://github.com/maggnus/ymp/commit/973a6e331f4577a894ab95ee325efa4a03005510).
