# Session resource budgets

YMP-102 implements admission in `Store::admit_invocation`. One SQLite `IMMEDIATE`
transaction checks captured limits, allocates the invocation ordinal, records the
assignment and token-usage row, and publishes a reservation event. Independent
connections cannot reserve the same remaining allowance. A denied admission
records a stable code and message without creating an invocation or charging a
turn. `begin_invocation` retains its explicit-ordinal API and uses the same guard.

The session policy captures `Limits.resources` once. Resume and follow-up use that
capture even when configuration changes. An older policy without resource fields
retains its recorded outer invocation, concurrency and timeout limits. A session
without any historical policy receives a separately labelled admission-limit
capture at its first resume; this does not invent historical policy or usage.

## Units and defaults

| Control | Default | Enforcement |
| --- | --- | --- |
| Session invocations, active invocations, invocation timeout | Existing `turns`, `parallel`, `turn_timeout_secs` | Shared admission and provider timeout |
| Initial planning and plan review | 6 invocations | Shared counter, including revisions and failures |
| Required review allowance | At least 2 invocations initially | Protected from other purposes; pending tasks and final review increase the required allowance |
| Supplied prompt plus profile instructions | 128,000 characters; 32,000 for startup | Reject before invocation; full transmitted prompt reference is recorded |
| Visible output | 64,000 characters per invocation | Stop after an observed stream or result exceeds the limit |
| Native conversation loop | 16 turns per invocation | Claude SDK `maxTurns`; Codex and ACP support is not claimed |
| Reported raw tokens | Optional `observed_tokens` and `invocation_tokens` together | Enforced admission ceiling and per-invocation reservation |

Startup samples at most two profiles, subject to concurrency and the remaining
startup allowance for plan review. It does not solicit the entire roster. Model
and effort selection and requested/sent/reported settings remain separate from
these resource controls. The defaults are operational bounds, not experimentally
calibrated optimal allocations.

Required task reviews and final review may consume protected capacity. Plan
review, bidding, communication, retries, execution, learning, synthesis and any
other purpose use the same session totals. Required review remains protected
while unresolved tasks remain; a completed native review call is not itself an
approval. Infeasible constraints stop work with preserved progress.

## Accounting and stopping

Observed raw input includes cache input; output already includes reasoning. The
existing native adapters aggregate internal requests where observable. Native
retries and context reprocessing therefore remain inside invocation usage; retry
events and partial/complete coverage are retained. There is no guessed request
count, currency conversion, provider grouping or estimate substituted for spend.

The active reservation is the unobserved remainder of each invocation's token
allowance. Admission adds observed tokens, those remainders, the new allowance
and protected review capacity. This avoids both double counting and granting two
assignments the same allowance. Terminal completion, failure, cancellation and
recovery release the remaining reservation; recorded invocation count and tokens
never reset. Storage rejects attempts to reduce previously observed input or
output. Missing usage stays missing.

With token admission enabled, any closed invocation with unavailable or partial
accounting blocks further admission, including required review. Without token
admission, those invocations still consume the supported counters and remain
visibly incomplete. The runtime requests cancellation when an observed token
ceiling is reached. Overshoot remains visible in `observed_token_overshoot`.

## Limits of the guarantee and client contract

Token reservations are not native hard token caps. A native request can exceed an
allowance before reporting usage; multiple requests and native retries may be
opaque. No finite token overshoot bound has been proved for the installed
backends. Native-loaded context is not bounded by the supplied-prompt character
limit, and a visible-output limit does not cap reasoning or generated tool data.
Timeouts bound provider futures, with process teardown handled by the existing
supervisor. These controls do not establish equal compute from invocation counts.

`Store::session_budget` and `SessionTrace.budget` expose captured limits, admitted
and active counters, startup use, protected review capacity, token reservations,
coverage, observed overshoot and the last admission denial or native resource stop. Reservation/release
and limit events link the invocation history; `budget_controls` records requested
native controls and their adapter limitation. `require_strict_token_bound`
rejects incomplete accounting and the unproved native hard-cap claim even when
reported totals are complete. Clients may report observed totals with their
coverage; they must not describe an opaque run as strictly token-budget-equal.

Offline evidence covers the actual engine, provider and SDK transport paths.
The initial ten-agent run failed the bounded-startup assertion with ten proposals.
Disabling admission produced three concurrent admissions where only two fit, and
allowed work after partial usage and overshoot. Removing native `maxTurns`
forwarding failed the physical SDK argument assertion. No paid provider inference
or credential reads are part of these checks.
