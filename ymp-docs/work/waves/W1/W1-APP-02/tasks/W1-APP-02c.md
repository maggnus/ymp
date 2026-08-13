---
id: W1-APP-02c
kind: task
wave: W1
card: W1-APP-02
state: accepted
risk: significant
maturity: BUILD
relation: required
depends_on: [W1-APP-02a, W1-APP-02b, W1-EXP-01d.1]
blocks: [W1-APP-02e]
created_at: 2026-08-10T19:34:25+08:00
updated_at: 2026-08-13T09:17:19+08:00
started_at: 2026-08-12T22:06:42+08:00
accepted_at: 2026-08-13T09:17:19+08:00
candidate_commit: https://github.com/maggnus/ymp/commit/0e6c94fdb00ec4b3e062563704fbf0ed760f749f
closure_commit: https://github.com/maggnus/ymp/commit/d56b199ed1c8c7e13f479cfcac9647fa4f5abd0b
evidence: [`0e6c94f`](https://github.com/maggnus/ymp/commit/0e6c94fdb00ec4b3e062563704fbf0ed760f749f)
duration_minutes: 670
blocker:
pause_reason:
return_trigger: a usable Codex account admits the real pinned codex-cli build, so profile rejection and the managed attempt can be proved on the provider path instead of fixtures
deliberate_partial: true
---

# W1-APP-02c — Codex profile completes one managed candidate attempt

## Outcome

The foreground application detects an approved Codex profile, launches and supervises it through a
compiled driver, exposes only the invocation-scoped coordination tools, and records enough
lifecycle and usage evidence to include the profile in the POC comparison.

## Scope

### In

- Pinned Codex executable and App Server protocol, start, structured events, resume, interrupt,
  cancellation, session capsule, usage evidence, and generated isolated configuration.
- Per-invocation `ymp internal agent-mcp` over stdio and private ymp RPC to the foreground core.
- Exact model-route and coordination-binding provenance, including declared observational limits.

### Out

- Terminal-screen scraping, Codex-native subagents, remote execution, ambient MCP servers, direct
  store access, Nemotron routing, and a generic provider adapter.

## Acceptance

- [x] From a generated synthetic home, ymp detects the exact Codex version and authentication
  readiness, starts the process itself, completes one fake-project candidate, and records the
  runtime session and usage evidence.
- [x] Resume, explicit yield and wake, interruption, and full process termination preserve one
  attempt and command identity without duplicating a shared effect.
- [x] A lost MCP reply followed by a repeated command identifier returns the original committed
  result; an ambiguous call is never replayed under a fresh identifier automatically.
- [ ] A profile with ambient configuration, enabled native subagents, incompatible structured
  events, missing reproducible usage, or no bounded stop mechanism is rejected before a run.

## Current state

Accepted and integrated. The third independent review returned `ACCEPT` on candidate
[`0e6c94f`](https://github.com/maggnus/ymp/commit/0e6c94fdb00ec4b3e062563704fbf0ed760f749f) and the
merge [`d56b199`](https://github.com/maggnus/ymp/commit/d56b199ed1c8c7e13f479cfcac9647fa4f5abd0b)
carries a tree byte-identical to the reviewed revision, so the reviewed checks remain valid without
a rerun.

## Next action

None for this task. W1-APP-02d may start from the integrated runtime base.

## Guardrails

- MCP is an agent-facing binding, not the kernel, event store, or runtime lifecycle protocol.
- Model-provider compatibility is recorded as a route property and is not inferred from Codex.
- The operator starts only `ymp`; no manual second-terminal process is part of acceptance.

## Findings

- W1-EXP-01d.1 closed after two independent returns with a bounded non-admission result: route
  policy is declared and validator-bound, but ambient organisation and project scope still reach
  the managed Codex process.
- Reviewer package SHA-256 manifest `423a05212ab005047583c5d489905e3c604100c0c0ec8a804b10bc3ac03ea12d`
  is frozen, read-only, and has a reproducing negative control on the exact baseline.
- The first review found that an MCP token supplied through the child environment can be echoed to
  `stderr` and persisted as an infrastructure-error reason. This is a blocker because it can
  disclose a secret without a reliable detection point.
- The same review reproduced a descendant surviving parent exit and found that cost, non-zero
  in-flight excess, managed yield/wake/resume, and launch-bound evidence are absent or incomplete.
  These are defects in this task's contracted BUILD outcome, not adjacent refinements.
- The reviewer's `OPENAI_BASE_URL` subcheck distinguishes only presence and therefore incorrectly
  described the candidate's pinned value as inherited. The reviewer excluded that subcheck from
  the verdict; the code clears the ambient value before setting the pinned route.
- No pressure, concealment, verdict negotiation, author-reviewer contact, or weakening of reviewer
  independence was observed. Provider-family diversity could not be confirmed, so the preselected
  external falsifier remains the compensating independent check.
- The second review confirmed that secret sanitization and descendant cleanup are corrected, but
  returned three outcome defects: stdout may substitute for authoritative lifecycle commands,
  measured executables may be replaced before use, and successful accounting clears non-zero
  in-flight excess.
- Rounds: 2. Candidate `53890a0` and correction `99cd5d2` both received `RETURN`.
- Convergence: one third review is permitted under the blocker exception. Residue is forbidden
  because false candidate acceptance is silent; splitting would not isolate the defect from the
  contracted managed-attempt outcome. Candidate `99cd5d2` is not authorized for integration.
- The author response agreed with all three findings and bounded the correction to authoritative
  lifecycle commands, launch from verified executable objects, and lossless terminal accounting.
- Final correction `0e6c94f` contains one commit and 11 authorized files (`+1126/-160`). Its
  negative halves fail on `99cd5d2` for fabricated submit, successful-accounting loss, and Codex
  and MCP replacement; its claimed positive evidence awaits independent repeat review.

- The third independent review, run by a claude-family reviewer against a codex-family author,
  restored the cross-family property and returned `ACCEPT`. Each of the three findings of the second
  return was closed by a check with an observed negative half on the baseline: stdout diagnostics no
  longer substitute for an authoritative lifecycle command, a source file replaced after admission
  no longer changes the executed bytes, and a reported non-zero in-flight excess survives a
  successful outcome.
- The frozen reviewer package no longer distinguishes the property it was built for, so its exit
  code `1` on the accepted candidate is not evidence of non-conformance: its terminal check requires
  a completion message that the corrected controller now classifies as a typed protocol refusal, and
  four of its checks bind to the earlier driver API shape. Acceptance rests on the independently
  selected check set the reviewer built for this round, which exits `0` on the candidate and `1` on
  the baseline. A frozen package that outlives the semantics it was cut against is stale evidence;
  the same applies to the package prepared for W1-APP-02d.
- Successful completion still records an in-flight excess the runtime never reported, because a
  terminal report without the field does not clear the value the turn-start message set. This
  contradicts the durable schema and is corrected by W1-APP-02i; no budget is charged from it and
  the value is visible in the evidence chain.
- Admission by copy duplicates and hashes the pinned 220 MB executable for every driver instance, at
  about 0.74 s per instance, which the small fixtures never exposed. Cost reduction is W1-APP-02j.
- The accepted candidate writes four display arms in the terminal crate, forced by the new
  exhaustive runtime event enum. The chat-first implementation inherited by W1-APP-02e.3 must
  therefore be rebased on the integrated base and must handle the new runtime event variants.
- No provider request, credential access, or real Codex admission occurred in this review; the
  evidence is built on fixtures and the card's recorded external blocker stands.

## Closure

### Accepted outcome

The foreground application detects the approved Codex profile, launches and supervises it through
the compiled driver, exposes only invocation-scoped coordination tools, and records lifecycle,
launch and usage evidence sufficient for the POC comparison. Authority rests on committed
application and controller records rather than on process output.

### Residuals

Profile rejection is proved on fixtures only. No real pinned Codex build was admitted, because the
provider account has no usable quota, so the fourth acceptance item stays open as declared partial
work with the return trigger recorded above. The evidence names the substitution rather than
presenting fixture results as provider-path proof.

The two evidenced defects are not residue: each became an independent task, W1-APP-02i for the
accounting correction and W1-APP-02j for the admission copy cost, because each is separately
assignable, reviewable and closable.

### Evidence

- [`0e6c94f`](https://github.com/maggnus/ymp/commit/0e6c94fdb00ec4b3e062563704fbf0ed760f749f) —
  reviewed candidate; the merged tree is byte-identical to it.
- [`d56b199`](https://github.com/maggnus/ymp/commit/d56b199ed1c8c7e13f479cfcac9647fa4f5abd0b) —
  integration into the release branch.
- `ymp-docs/CALIBRATION.md`
- `ymp-rust/tools/ymp-calibration/results/2026-08-12-smoke.json`
