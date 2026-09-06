---
id: W2-TUI-03f
kind: task
wave: W2
card: W2-TUI-03
state: review
risk: significant
maturity: BUILD
relation: required
depends_on: []
blocks: [W2-TUI-03b]
created_at: 2026-09-06T10:41:19+08:00
updated_at: 2026-09-06T10:41:19+08:00
started_at: 2026-09-06T10:45:28+08:00
accepted_at:
candidate_commit: https://github.com/maggnus/ymp/commit/3fa266e98914d3b904d4a9fc8a8ab54b87a8fc7b
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

# W2-TUI-03f — Command output retains the complete conversation without a viewport

## Outcome

A one-shot CLI request prints its complete finite conversation in order, including the original request and the unique drafted-contract confirmation even when the verifier makes the response longer than the interactive viewport. The interactive English TUI retains its reviewed bounded scrolling. Prose assertions tolerate wrapping without tolerating missing content.

## Scope

### In

Origin: the independent integration consultation found a real cross-component regression after accepted frame candidate 0af2354. CLI print_transcript calls ui::surface_spec with a fixed 120x40 viewport; a's narrower text produces more lines, and the viewport silently drops the start of a generated-verifier conversation. No task may land that integration loss merely because the CLI consumer is outside a's original write zone.

Start from the CTO-provided isolated combined candidate containing reviewed a/d, the English reference and fixture e. Its full SHA is supplied at dispatch. This task has no accepted-node prerequisite: it corrects the candidate before integration. Read AGENTS, this task, the CLI print path and ui transcript composition, plus the retained integration consultation.

Exclusive write zone (five files): ymp-rust/crates/ymp-cli/src/surface.rs; ymp-rust/crates/ymp-cli/tests/generated_verifier.rs; ymp-rust/crates/ymp-cli/tests/pool_surfaces.rs; ymp-rust/crates/ymp-cli/tests/state_lives_under_the_home_root.rs; ymp-rust/crates/ymp-tui/src/ui.rs. The shared ui file may expose a minimal complete-transcript path if needed; do not redesign a's interactive layout.

### Out

No changed domain effects, authorization, budget, provider readiness/disclosure, command inventory, storage, verifier logic, runtime, corpus manifests, dependencies, design or other tests. No real models or full workspace suite. Other integration failures are separate findings; do not fix them in this zone.

## Acceptance

- [ ] The actual CLI request path emits the first, intermediate and last semantically significant lines of a long generated-verifier conversation, with exactly one drafted-contract confirmation, complete verifier text and preserved Unicode input. An arbitrary enlarged terminal height or a truncation marker alone does not satisfy complete output.
- [ ] The CLI and interactive path share truthful projection and sanitization; interactive viewport/scrolling behavior remains unchanged. No terminal control sequence is made executable by the complete-output path.
- [ ] Whitespace normalization is limited to prose comparisons in the two named CLI test helpers. Expectations still fail for removed content, altered counts, incorrect readiness, missing refusal or path disclosure; raw-byte/layout/sanitization checks are not weakened. A latent failure revealed after normalization must be reported, not masked.
- [ ] The three named CLI test targets pass where this task changes their behavior; any surviving failure must be exactly demonstrated to be independent, recorded and returned to CTO. The original generated_verifier failure discriminates before/after; at least one stronger long-response check fails with clipping restored and succeeds with the fix.
- [ ] Narrow affected CLI/TUI tests, formatting, Clippy and diff checks pass. Retain reproducible output in a fresh short /tmp evaluation root with isolated project, HOME, YMP_HOME, TMPDIR, build and exports. Do not run the full workspace suite.

## Current state

Candidate 3fa266e is under independent review after the complete-output positive/negative proof, twelve pool/state tests and narrow TUI checks passed. The pre-existing standing-hint assertion remains a separate failure. a has a second substantive return queued after f; no concurrent writer remains in ui.rs.

## Next action

Complete independent review of 774f476..3fa266e, then supply the accepted candidate to the queued a status correction.

## Guardrails

Use shared production composition rather than a second interpretation of journal facts. All application-authored text is English and user/attributed Unicode remains verbatim. Do not change the scientific admission freeze or treat infrastructure failures as capability evidence. Preserve evidence for independent review.

## Findings

The reviewer called the consumer loss independent of a's file zone. CTO treats it as an integration regression that must close before landing; f is the explicit owner of the additional CLI boundary. Raw prose assertions in pool_surfaces and state_lives_under_the_home_root also encode old wrapping. Consultation used source and retained output, not a new executable proof.

## Review rounds

Recorded by the CTO ledger.

## Closure

### Accepted outcome

Pending.

### Residuals

Pending.

### Evidence

Pending.
