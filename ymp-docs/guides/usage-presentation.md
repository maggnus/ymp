# Usage presentation: new work, cache traffic and billing

Owner direction on 2026-09-14: avoid presenting large repeated cache traffic as if
it were the paid expense. The UI must remain accurate. The current model stores
token observations, not a trustworthy per-session payment or currency record.

The [OpenAI model pricing reference](https://developers.openai.com/api/docs/models/compare)
charges cached input separately from ordinary input. The
[Codex pricing guide](https://learn.chatgpt.com/docs/pricing) distinguishes included
plan usage, credits and API-key pricing; its credit table also assigns different
rates to cached input. These sources do not establish the owner's actual invoice
or tariff. Do not claim that cached tokens are free or that subtracting them gives
an amount actually paid. A cross-model token count is not a common monetary unit.

## Proposed primary UI quantity

Present new input and output prominently, either as two concise labeled values or
a clearly named new-token subtotal. New input here means canonical input not
served from cache; new cache creation remains new input. Output already includes
reasoning. Use a label such as `New tokens` or explicit `Uncached input` and
`Output`; never label this derived count `Paid`, `Billed`, `Cost` or `Spend`.
Retain cache-read, cache-write and processed input+output totals in details.

For the current Luna application records the derived illustration is 258,106 new
input plus 88,216 output, totaling 346,322, rather than the 11,172,306 processed total.
The 10,825,984 cache-read tokens remain recorded and inspectable. Native accounting
verification is still in progress; this illustration is not a payment estimate.

Derive the quantity only from sufficiently complete per-invocation observations.
A missing cache count is not zero; current aggregate TokenCounts::add_assign can
retain a known partial sum when other calls lack the field, so subtracting the
aggregate fields blindly does not prove a complete new-token total. Preserve
coverage, reject inconsistent negative differences and clearly mark partial or
unknown results. Incomplete GLM last-request-only observations stay partial.
Reasoning and cache fields must never be counted twice.

Keep native records, history, JSON/MCP exports, accounting and budget enforcement
unchanged. Existing processed-token limits include cache and must remain clearly
labeled in the relevant details/limits views. Do not compare the new headline to
a differently defined budget or silently change what stops a session.

## Monetary information

Display an actual currency/credit debit only when a trustworthy source supplies
that observation and its scope. A separately calculated API-rate equivalent is
an estimate and needs an explicit label, exact rate/version and coverage; it is
not the charge under a native subscription. Missing price, tariff or usage is
unknown, never zero. No new pricing table, account scraper, billing connector or
payment calculation is authorized by this presentation slice.

This contract translates the owner's readability goal into quantities the current
application can substantiate. UI implementation is delegated to Claude after 146
integration, alongside the confirmed transcript/progress corrections in 156.
