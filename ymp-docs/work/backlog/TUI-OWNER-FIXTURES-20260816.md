# Owner-reported interface failures (2026-08-16, verbatim transcripts)

Recorded by the CTO from the owner's live session on `ymp 1.0.4`, host `/Users/maggnus/Downloads/_ymp`.
These are acceptance fixtures for the planned TUI-interaction design revision; they are reproduced
verbatim so the revision argues against what the owner actually saw, not a paraphrase.

## Session A — the "ping" draft path

The owner typed `ping` as a goal. Three stacked application replies followed.

1. **Pool-state reply placed before the draft exists.**

```
ymp  your goal is held · this host offers models and holds no pool to draw them from, which is a state it should never reach — /providers · r measures the account again and resolves the pools · nothing has started and nothing has left
       this host
```

2. **A second reply restates mechanics the operator never asked about.**

```
ymp  request recorded locally — nothing has started and nothing is spent. Assembling a contract from this project: a copy of it as the negative control, and a verifier proposed from the way it runs its tests or, when it runs none, derived from the request itself.
```

3. **The refusal is a four-line wall of internal reasoning.**

```
ymp  ✗ nothing in the request "ping" names a result this host could check for: it asks for no artifact ymp knows how to look for, and this project runs no tests to propose a verifier from. A check invented from words that state no observable result would decide nothing, so none is offered — state a verifier of your own, or say what the work must leave behind. Nothing was recorded. Amend the draft — for example `verifier <path>` — or state the work again.
```

Owner verdict on the path: «кошмар какой-то».

## Session B — /providers

```
/providers

transcript › providers(all)[2] · 1 enabled · 1 ready
 PROVIDER    STATE           REACHED BY              MODELS  OBSERVED
 anthropic   disabled        claude-code 2.1.233 (Cl…55      1h 46m ago
   ↳ disabled from the provider view
 openai      ready           codex codex-cli 0.147.0 1       1h 46m ago

  this is the whole supported list, whether or not anything is configured
  nothing about a provider is measured before it is enabled: opening this table starts no process and reaches no network
  Enter opens a provider, where what enabling permits is stated above the key that takes the decision
```

Owner verdict: «/providers - тоже хрень какая-то».

## CTO reading (diagnosis, not yet the fix)

A. **Transcript verbosity**: one operator turn produced three separate application replies, each
   restating internal machinery (pools, contract assembly, verifier derivation). The operator's
   question was `ping`; the product answered with its architecture. Expected shape: one short
   reply, or none.

B. **The refusal wall**: the ✗ message is ~70 words of causal reasoning for a case whose fix is
   one clause — "say what `ping` should leave behind". Refusals must lead with the operator's
   next action, not the system's epistemology.

C. **Table density**: `/providers` spends four lines on notes that are static documentation, not
   state; `REACHED BY` truncates mid-word `(Cl…55`; a `MODELS` count of 1 sits beside an empty
   `MODELS` cell for the disabled row; `1h 46m` staleness is noise for a disabled provider. The
   four-line note block should be one line or moved behind `?`.

D. **Contradiction to investigate**: reply 1 claims "holds no pool to draw them from — a state it
   should never reach", while `/providers` shows openai `ready` with 1 model. If a ready provider
   exists but no pool was reconciled, the reconcile wiring (W1-PRD-05f, under review when this
   session ran) is the suspected missing piece — a functional bug, not only a wording problem.

E. **State vocabulary**: `disabled` appears both as STATE and as a `↳` fix line; `1 enabled ·
   1 ready` in the breadcrumb double-counts against a 2-row table (enabled≠ready distinction is
   unclear to the operator).
