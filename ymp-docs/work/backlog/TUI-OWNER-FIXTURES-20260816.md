# Owner-reported interface failures (2026-08-16, verbatim transcripts)

Recorded by the CTO from the owner's live session on `ymp 1.0.4`, host `/Users/maggnus/Downloads/_ymp`.
These are acceptance fixtures for the TUI-interaction design revision; they are reproduced verbatim
so the revision argues against what the owner actually saw, not a paraphrase.

Design revision 1 (2026-08-16) is recorded in
[`design/ymp_chat_tui.dc.html`](../../design/ymp_chat_tui.dc.html), section «Ревизия 1». Every
screen form quoted here is marked below as **rejected**, with one line of why; the rejected form is
part of the contract, because it explains why the accepted form looks the way it does.

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

## Session C — /models and /runtimes (screen captures, same session)

`design/Screenshot 2026-08-16 at 12.38.57 AM.png` — every one of 56 model rows carries the same
`↳ anthropic is not enabled — disabled from the provider view`, and `claude-3-5-sonnet-20241022-…`
runs into `anthropic` with no gap. `design/Screenshot 2026-08-16 at 12.39.42 AM.png` — the fix line
is cut off at the right edge mid-word, and the note block spells out all 55 model names over ten
lines.

## Resolution — what was rejected and what replaces it

### A · Transcript verbosity

One operator turn produced three application replies, each restating internal machinery. The cold
start spent five lines on an ASCII logo and eleven more on mechanics.

- **Rejected — three replies to one turn.** The operator asked one thing and was answered three
  times, twice about machinery that was not asked about.
- **Rejected — five-line ASCII logo plus an eleven-line opening block.** The first screen has to
  state where you are and what to type; everything else belongs behind `?`.
- **Rejected — event continuation lines indented two columns past the text column.** A wrapped line
  that does not sit under the line it continues reads as a separate record.
- **Rejected — `sha256:9c41f2…` in tables and in the transcript.** Seven characters of algorithm
  prefix and a 26-column cell carry six significant characters.
- **Accepted:** banner of exactly three lines with the invitation as the third
  ([`#1a`](../../design/ymp_chat_tui.dc.html), [`#2b`](../../design/ymp_chat_tui.dc.html)); one
  operator turn, one reply; text starts in a fixed column — after a 19-character prefix for events
  and a 7-character one for replies — and wrapped lines sit under it; digests are six characters,
  the full form only in `describe`.

### B · The refusal wall

Roughly seventy words of causal reasoning for a case whose fix is one clause.

- **Rejected — the four-line ✗ message quoted in Session A.** It explains the system's epistemology
  before it names what the operator should do, and the action arrives in the last sentence.
- **Accepted** ([`#2a`](../../design/ymp_chat_tui.dc.html)): a ✗ reply is at most two lines, the
  first begins with the operator's action, and the mechanics live behind `:describe refusal`.

### C · Table density

Four lines of static documentation under a two-row table; `REACHED BY` truncated into the next
column; a repeated per-row `↳` that states the same thing for every row.

- **Rejected — the four-line note block under the table.** It is documentation, not state, and it
  is re-read on every visit.
- **Rejected — `(Cl…55`.** Truncation that ends flush against the next column produces a value that
  cannot be read as either field.
- **Rejected — the same `↳` line under every row.** A reason shared by all rows is a fact about the
  table, and belongs in its header once.
- **Rejected — left-aligned counts (`55`, `1`) in a numeric column.** Digits that do not line up
  cannot be compared down the column.
- **Accepted** ([`#2g`](../../design/ymp_chat_tui.dc.html),
  [`#2c`](../../design/ymp_chat_tui.dc.html)): widths derive from content up to a per-column cap;
  a truncated value keeps at least one space before the next column; numbers are right-aligned;
  `/state:ready` filters one column; the note under a table is at most one line; key hints sit in
  the status line as `e enable · d disable · Enter properties · ? more`. Enabling is a consent
  action and opens a modal whose consequence is stated on the line directly above the key
  (decision D4, [`#2e`](../../design/ymp_chat_tui.dc.html)); reversible actions stay single keys and
  irreversible ones keep the typed confirmation
  ([`#2h`](../../design/ymp_chat_tui.dc.html)).

### D · Pool state contradicted the provider table

Reply 1 claimed "this host holds no pool to draw them from — a state it should never reach", while
`/providers` showed openai `ready` with one model.

- **Rejected — «this host holds no pool» while a provider reads `ready`.** The sentence is only
  true when every provider is `off`; anywhere else it contradicts the table the operator can open.
- **Accepted** ([`#2f`](../../design/ymp_chat_tui.dc.html),
  [`#2i`](../../design/ymp_chat_tui.dc.html)): a pool is `measuring` or `ready` as long as any
  provider is `ready`, `/pools` states which providers each pool draws from, and the transcript
  reply reads that same state rather than restating it.
- **Still open, and not a wording defect:** whether the reconciler actually produces a pool from a
  ready provider is functional wiring (`W1-PRD-05f`) and is not decided by this revision.

### E · State vocabulary

`disabled` appeared both as a STATE value and inside a `↳` fix line, and the breadcrumb
`1 enabled · 1 ready` counted two words against two rows.

- **Rejected — `enabled` as a state.** Enabling is an operator action; what follows it is
  `ready`, `measuring` or `error`.
- **Rejected — a breadcrumb that mixes two vocabularies.** `1 enabled · 1 ready` cannot be checked
  against a two-row table, because a row can be counted twice or not at all.
- **Rejected — `disabled` repeated as a `↳` line under a `disabled` row.** The fix line repeats the
  state instead of naming an action.
- **Accepted** ([`#2i`](../../design/ymp_chat_tui.dc.html)): one vocabulary — `off`, `ready`,
  `measuring`, `error` — for providers, pools and runs; the header counts in that vocabulary only,
  and the counters sum to the number of rows (`providers(all)[5] · 2 ready · 1 measuring · 1 error ·
  1 off`). The five terminal run outcomes are untouched by it, and the surface-state markers
  (`loading`, `empty`, `stale`, `degraded`, `error`) remain a separate axis.
